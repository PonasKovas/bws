use anyhow::{bail, Context, Result};
use async_ffi::{FfiContext, FfiFuture, FfiPoll};
use bws_plugin::stable_types::global_state::plugins::{BwsPlugin, BwsPlugins};
use bws_plugin::*;
use libloading::{Library, Symbol};
use log::{error, info};
use semver::{Version, VersionReq};
use sha2::digest::generic_array::transmute;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::mem::{swap, ManuallyDrop};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    path::Path,
    sync::Arc,
};
use tokio::fs;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::RwLock;

use crate::global_state::{GlobalState, InnerGlobalState};
use crate::plugins;
use crate::shared::LinearSearch;

const ABI_VERSION: u64 = ((async_ffi::ABI_VERSION as u64) << 32) | crate::ABI_VERSION as u64;

pub struct Plugin {
    gate: Option<Gate<PluginEvent<'static>>>, // None if the plugin is not active
    plugin: PluginData,
}

pub struct PluginData {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, VersionReq)>,
    pub subscribed_events: [u8; (PluginEvent::VARIANT_COUNT + 7) / 8], // +7 so it would round up
    pub arbitrary_subscribed_events: HashSet<String>,
    pub library: Arc<Library>,
    pub entry: _f_PluginEntry,
}

struct CandidatePlugin {
    name: String,
    version: Version,
    dependencies: Vec<(String, String)>,
    subscribed_events: [u8; (PluginEvent::VARIANT_COUNT + 7) / 8],
    arbitrary_subscribed_events: HashSet<String>,
    entry: _f_PluginEntry,
    subplugins: Vec<SubPluginData>,
}

struct SubPluginData {
    name: String,
    subscribed_events: [u8; (SubPluginEvent::VARIANT_COUNT + 7) / 8],
    arbitrary_subscribed_events: HashSet<String>,
    entry: _f_SubPluginEntry,
}

// T is event enum, either plugin event or subplugin event
pub struct Gate<T: Sized> {
    sender: mpsc::UnboundedSender<Tuple2<T, SendPtr<oneshot::Sender<()>>>>,
}

impl<T: Sized> Gate<T> {
    pub fn new() -> (
        Self,
        &'static mut mpsc::UnboundedReceiver<Tuple2<T, SendPtr<oneshot::Sender<()>>>>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();

        (Self { sender }, Box::leak(Box::new(receiver)))
    }
    // If None, the plugin died while handling the call or was already dead
    pub async fn call(&self, message: T) -> Option<()> {
        let (sender, receiver) = oneshot::channel();
        let sender = ManuallyDrop::new(sender);
        if let Err(_) = self
            .sender
            .send(Tuple2(message, SendPtr(&*sender as *const _)))
        {
            return None;
        }
        receiver.await.ok()
    }
}

unsafe extern "C" fn recv_plugin_event(
    receiver: *const (),
    ctx: &mut FfiContext,
) -> FfiPoll<BwsOption<Tuple2<PluginEvent<'static>, *const ()>>> {
    // this catch_unwind is useless because the panic hook still triggers and the tokio runtime immediatelly shutdown
    // without the plugin printing the stacktrace
    // TODO do something about this
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let receiver: &mut mpsc::UnboundedReceiver<Tuple2<PluginEvent, *const ()>> =
            transmute(receiver);
        match ctx.with_context(|ctx| receiver.poll_recv(ctx)) {
            std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
            std::task::Poll::Pending => FfiPoll::Pending,
        }
    })) {
        Ok(p) => p,
        Err(_) => FfiPoll::Panicked,
    }
}

unsafe extern "C" fn send_oneshot(sender: BwsOneshotSender) {
    let sender = std::ptr::read(*sender as *const _ as *const oneshot::Sender<()>);
    if sender.send(()).is_err() {
        error!("Error completing event call.");
    }
}

pub async fn start_plugins(global_state: &GlobalState) -> Result<()> {
    let plugins = &global_state.plugins;

    // Use the graph theory to order the plugins so that they would load only after all of their dependencies
    // have loaded.
    let mut graph = petgraph::graph::DiGraph::<String, ()>::new();
    let mut indices = Vec::new();
    for plugin in plugins {
        indices.push((plugin.0.clone(), graph.add_node(plugin.0.clone())));
    }
    // set the edges
    for plugin in plugins {
        let pid = indices.search(plugin.0);
        for dependency in &plugin.1.read().await.plugin.dependencies {
            graph.update_edge(*indices.search(&dependency.0), *pid, ());
        }
    }

    let ordering = match petgraph::algo::toposort(&graph, None) {
        Ok(o) => o,
        Err(cycle) => {
            bail!(
                "Dependency cycle detected: {}",
                indices.search_by_val(&cycle.node_id())
            );
        }
    };

    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        let plugin = &plugins[plugin_name];

        // Create the gate
        let (gate, receiver) = Gate::new();

        tokio::spawn(unsafe {
            (plugin.read().await.plugin.entry)(
                BwsStr::from_str(plugin_name),
                PluginGate::new(
                    receiver as *const _ as *const (),
                    recv_plugin_event,
                    send_oneshot,
                ),
                BwsGlobalState::new(Arc::into_raw(Arc::clone(global_state)) as *const (), {
                    use bws_plugin::stable_types::global_state::{
                        plugins::{PluginVTable, PluginsIterVTable, PluginsVTable},
                        GlobalStateVTable,
                    };
                    unsafe extern "C" fn drop(ptr: *const ()) {
                        std::mem::drop(Arc::from_raw(ptr as *const _));
                    }
                    unsafe extern "C" fn get_compression_treshold(ptr: *const ()) -> i32 {
                        (*(ptr as *const InnerGlobalState)).compression_treshold
                    }
                    unsafe extern "C" fn get_port(ptr: *const ()) -> u16 {
                        (*(ptr as *const InnerGlobalState)).port
                    }
                    unsafe extern "C" fn get_plugins(
                        ptr: *const (),
                    ) -> Tuple2<*const (), PluginsVTable> {
                        unsafe extern "C" fn get_plugin(
                            ptr: *const (),
                            name: BwsStr,
                        ) -> BwsOption<Tuple2<*const (), PluginVTable>> {
                            BwsOption::from_option(
                                (*(ptr as *const HashMap<String, RwLock<Plugin>>))
                                    .get(name.as_str())
                                    .map(|plugin| {
                                        Tuple2(plugin as *const _ as *const (), PluginVTable {})
                                    }),
                            )
                        }
                        unsafe extern "C" fn iter(
                            ptr: *const (),
                        ) -> Tuple2<*const (), PluginsIterVTable> {
                            Box::into_raw(Box::new(
                                (*(ptr as *const HashMap<String, RwLock<Plugin>>)).iter(),
                            ));
                            todo!()
                        }
                        Tuple2(
                            &(*(ptr as *const InnerGlobalState)).plugins as *const _ as *const (),
                            PluginsVTable {
                                get: get_plugin,
                                iter: iter,
                            },
                        )
                    }

                    GlobalStateVTable {
                        drop,
                        get_compression_treshold,
                        get_port,
                        get_plugins,
                    }
                }),
            )
        });

        plugin.write().await.gate = Some(gate);

        info!(
            "Plugin {:?} loaded and started. (Provided by {:?})",
            plugin_name,
            plugin.read().await.plugin.provided_by
        );
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let mut response = false;
    let event = PluginEvent::Arbitrary(
        BwsStr::from_str("hello"),
        SendMutPtr(&mut response as *mut _ as *mut ()),
    );
    match plugins["test_plugin"]
        .read()
        .await
        .gate
        .as_ref()
        .unwrap()
        .call(event)
        .await
    {
        Some(()) => {
            info!("Event 'hello' response: {}", response);
        }
        None => {
            error!("Error calling event 'hello'");
        }
    }

    Ok(())
}

pub async fn load_plugins() -> Result<HashMap<String, Plugin>> {
    let mut plugins: HashMap<String, Plugin> = HashMap::new();

    let mut read_dir = fs::read_dir("plugins").await?;
    while let Some(path) = read_dir.next_entry().await? {
        let path = path.path().canonicalize()?;

        // ignore directories
        if path.is_dir() {
            continue;
        }
        match path.file_name().unwrap().to_str() {
            // also skip files with invalid unicode in their names
            None => continue,
            Some(path) => {
                // skip hidden files
                if path.starts_with('.') {
                    continue;
                }
            }
        }

        if let Err(e) = unsafe { load_library(&mut plugins, &path).await } {
            error!("Error loading {:?}: {:?}", path.file_name().unwrap(), e);
        }
    }

    // check if all dependencies of plugins are satisfied
    for plugin in &plugins {
        match check_dependencies(plugin.0, &plugins) {
            Ok(true) => {}
            Ok(false) => {
                bail!(
                    "Plugin {:?} could not be loaded, because it's dependencies weren't satisfied. (Provided by {:?})",
                    plugin.0, plugins[plugin.0].plugin.provided_by
                );
            }
            Err(e) => {
                bail!(
                    "Error reading dependencies of plugin {:?} (Provided by {:?}): {:?}",
                    plugin.0,
                    plugins[plugin.0].plugin.provided_by,
                    e
                );
            }
        }
    }

    Ok(plugins)
}

async unsafe fn load_library(
    plugins: &mut HashMap<String, Plugin>,
    path: impl AsRef<Path>,
) -> Result<()> {
    let path = path.as_ref();
    let lib = Arc::new(Library::new(path)?);

    let abi_version: Symbol<*const u64> = lib.get(b"BWS_ABI_VERSION")?;

    if **abi_version != ABI_VERSION {
        bail!(
        	"plugin is compiled with a non-compatible ABI version. BWS uses {}, while the library was compiled with {}.",
        	ABI_VERSION,
        	**abi_version
        );
    }

    // register the plugins

    static mut TO_REGISTER: Vec<CandidatePlugin> = Vec::new();

    unsafe extern "C" fn register_plugin(
        name: BwsStr,
        version: Tuple3<u64, u64, u64>,
        dependencies: BwsSlice<Tuple2<BwsStr, BwsStr>>,
        entry: _f_PluginEntry,
    ) -> Tuple2<_f_PluginSubscribeToEvent, _f_RegisterSubPlugin> {
        TO_REGISTER.push(CandidatePlugin {
            name: name.as_str().to_owned(),
            version: Version::new(version.0, version.1, version.2),
            dependencies: dependencies
                .as_slice()
                .iter()
                .map(|e| (e.0.as_str().to_owned(), e.1.as_str().to_owned()))
                .collect(),
            subscribed_events: [0; (PluginEvent::VARIANT_COUNT + 7) / 8],
            arbitrary_subscribed_events: HashSet::new(),
            entry,
            subplugins: Vec::new(),
        });

        Tuple2(plugin_subscribe_to_event, register_subplugin)
    }

    unsafe extern "C" fn register_subplugin(
        name: BwsStr,
        entry: _f_SubPluginEntry,
    ) -> _f_SubPluginSubscribeToEvent {
        let plugin = TO_REGISTER.last_mut().unwrap();

        plugin.subplugins.push(SubPluginData {
            name: name.as_str().to_owned(),
            subscribed_events: [0; (SubPluginEvent::VARIANT_COUNT + 7) / 8],
            arbitrary_subscribed_events: HashSet::new(),
            entry,
        });

        subplugin_subscribe_to_event
    }

    unsafe extern "C" fn plugin_subscribe_to_event(event_name: BwsStr) {
        let plugin = TO_REGISTER.last_mut().unwrap();

        match event_name.as_str() {
            other => {
                plugin.arbitrary_subscribed_events.insert(other.to_owned());
            }
        }
    }

    unsafe extern "C" fn subplugin_subscribe_to_event(event_name: BwsStr) {
        let plugin = TO_REGISTER.last_mut().unwrap();

        let subplugin = plugin.subplugins.last_mut().unwrap();

        match event_name.as_str() {
            other => {
                subplugin
                    .arbitrary_subscribed_events
                    .insert(other.to_owned());
            }
        }
    }

    let plugin_registrator: Symbol<unsafe extern "C" fn(_f_RegisterPlugin)> =
        lib.get(b"bws_load_library")?;

    (*plugin_registrator)(register_plugin);

    let mut to_register_non_static = Vec::new();
    swap(&mut TO_REGISTER, &mut to_register_non_static);
    for plugin in to_register_non_static {
        plugins.insert(
            plugin.name,
            Plugin {
                gate: None,
                plugin: PluginData {
                    version: plugin.version,
                    provided_by: path.to_path_buf(),
                    dependencies: plugin
                        .dependencies
                        .into_iter()
                        .try_fold::<_, _, Result<_>>(Vec::new(), |mut acc, dep| {
                            acc.push((
                                dep.0,
                                VersionReq::parse(&dep.1)
                                    .context("error parsing version requirement")?,
                            ));

                            Ok(acc)
                        })?,
                    subscribed_events: plugin.subscribed_events,
                    arbitrary_subscribed_events: plugin.arbitrary_subscribed_events,
                    entry: plugin.entry,
                    library: Arc::clone(&lib),
                },
            },
        );
    }

    Ok(())
}

fn check_dependencies(plugin_name: &str, plugins: &HashMap<String, Plugin>) -> Result<bool> {
    let mut result = true;

    let plugin = &plugins[plugin_name].plugin;
    for dependency in &plugin.dependencies {
        match plugins.get(&dependency.0) {
            Some(dep_plugin) => {
                if !dependency.1.matches(&dep_plugin.plugin.version) {
                    error!(
                        "Plugin {:?} dependency {:?} {} was not met. {:?} {} is present, but does not match the {} version requirement.",
                        plugin_name, dependency.0, dependency.1, dependency.0, dep_plugin.plugin.version, dependency.1
                    );
                    result = false;
                }
            }
            None => {
                error!(
                    "Plugin {:?} dependency {:?} {} was not met.",
                    plugin_name, dependency.0, dependency.1
                );
                result = false;
            }
        }
    }
    Ok(result)
}
