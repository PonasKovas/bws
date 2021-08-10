use anyhow::{bail, Context, Result};
use async_ffi::{FfiContext, FfiFuture, FfiPoll};
use bws_plugin::*;
use libloading::{Library, Symbol};
use log::{error, info};
use semver::{Version, VersionReq};
use sha2::digest::generic_array::transmute;
use std::collections::HashSet;
use std::mem::swap;
use std::path::PathBuf;
use std::ptr::null;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    path::Path,
    sync::Arc,
};
use tokio::fs;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::plugins;

const ABI_VERSION: u64 = ((async_ffi::ABI_VERSION as u64) << 32) | crate::ABI_VERSION as u64;

pub type Plugins = HashMap<String, Plugin>;
pub struct Plugin {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, String)>,
    pub gate: Gate<PluginEvent>,
    pub subscribed_events: [u8; (PluginEvent::VARIANT_COUNT + 7) / 8], // +7 so it would round up
    pub arbitrary_subscribed_events: HashSet<String>,
    pub subplugins: Vec<SubPlugin>,
    pub entry: Option<FfiFuture<Unit>>,
}
pub struct SubPlugin {
    pub name: String,
    pub gate: Gate<SubPluginEvent>,
    pub subscribed_events: [u8; (SubPluginEvent::VARIANT_COUNT + 7) / 8], // +7 so it would round up
    pub arbitrary_subscribed_events: HashSet<String>,
    pub entry: Option<FfiFuture<Unit>>,
}

// T is event enum, either plugin event or subplugin event
pub struct Gate<T: Sized> {
    sender: mpsc::UnboundedSender<Tuple2<T, *const oneshot::Sender<bool>>>,
}

impl<T: Sized> Gate<T> {
    pub fn new() -> (
        Self,
        &'static mut mpsc::UnboundedReceiver<Tuple2<T, *const oneshot::Sender<bool>>>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();

        (Self { sender }, Box::leak(Box::new(receiver)))
    }
    // false = execute default behaviour
    pub async fn call(&mut self, message: T) -> bool {
        let (sender, receiver) = oneshot::channel();
        if let Err(_) = self.sender.send(Tuple2(message, &sender as *const _)) {
            return false;
        }
        receiver.await.unwrap_or(false)
    }
}

unsafe extern "C" fn recv_plugin_event(
    receiver: PluginEventReceiver,
    cx: &mut FfiContext,
) -> FfiPoll<BwsOption<Tuple2<PluginEvent, OneshotSender>>> {
    let receiver: &mut mpsc::UnboundedReceiver<Tuple2<PluginEvent, OneshotSender>> =
        transmute(receiver.0);
    match receiver.poll_recv(&mut cx.into_context()) {
        std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
        std::task::Poll::Pending => FfiPoll::Pending,
    }
}

pub async fn load_plugins() -> Result<Plugins> {
    let mut plugins = Plugins::new();

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

    // check the plugins and remove any that have unsatisfied dependencies
    let mut any_removed = false;
    // loop because a single pass might not be enough, if for example P1 depends on P2 which depends on P3.
    // And P3 is not present. On the first pass, P1 would be loaded fine, and P2 would be removed because of
    // unsatisfied dependencies, invalidating P1 at the same time, so we need an additional pass to remove P1 too.
    loop {
        let keys_to_remove = plugins
        .keys()
        .filter(|k| match check_dependencies(k, &plugins) {
            Ok(true) => {
                false
            }
            Ok(false) => {
                error!(
                    "Plugin {:?} could not be loaded, because it's dependencies weren't satisfied. (Provided by {:?})",
                    k, plugins[*k].provided_by
                );
                true
            }
            Err(e) => {
                error!(
                    "Error reading dependencies of plugin {:?} (Provided by {:?}): {:?}",
                    k,
                    plugins[*k].provided_by,
                    e
                );
                true
            }
        })
        .cloned()
        .collect::<Vec<_>>();
        for key in keys_to_remove {
            plugins.remove(&key);
            any_removed = true;
        }

        if !any_removed {
            break;
        }
    }

    for plugin in &mut plugins {
        if let Some(entry) = plugin.1.entry.take() {
            tokio::spawn(entry);
        }
        info!(
            "Plugin {:?} loaded. (Provided by {:?})",
            plugin.0, plugin.1.provided_by
        );
    }

    Ok(plugins)
}

async unsafe fn load_library(plugins: &mut Plugins, path: impl AsRef<Path>) -> Result<()> {
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

    static mut PATH: *const &Path = null();
    PATH = transmute(&path);
    static mut TO_REGISTER: Vec<(String, Plugin)> = Vec::new();

    unsafe extern "C" fn register_plugin(
        name: BwsStr,
        version: Tuple3<u64, u64, u64>,
        dependencies: BwsSlice<Tuple2<BwsStr, BwsStr>>,
    ) -> Tuple5<
        RecvPluginEvent,
        PluginEventReceiver,
        PluginEntry,
        PluginSubscribeToEvent,
        RegisterSubPlugin,
    > {
        let (gate, receiver) = Gate::new();
        TO_REGISTER.push((
            name.into_str().to_owned(),
            Plugin {
                version: Version::new(version.0, version.1, version.2),
                provided_by: (*PATH).to_path_buf(),
                dependencies: dependencies
                    .into_slice()
                    .iter()
                    .map(|e| (e.0.into_str().to_owned(), e.1.into_str().to_owned()))
                    .collect(),
                gate,
                subscribed_events: [0; (PluginEvent::VARIANT_COUNT + 7) / 8],
                arbitrary_subscribed_events: HashSet::new(),
                subplugins: Vec::new(),
                entry: None,
            },
        ));

        Tuple5(
            recv_plugin_event,
            PluginEventReceiver((receiver as *const _ as *const ()).as_ref().unwrap()),
            plugin_entry,
            plugin_subscribe_to_event,
            register_subplugin,
        )
    }

    unsafe extern "C" fn plugin_entry(future: FfiFuture<Unit>) {
        let plugin = TO_REGISTER.last_mut().unwrap();

        plugin.1.entry = Some(future);
    }

    unsafe extern "C" fn plugin_subscribe_to_event(event_name: BwsStr) {
        let plugin = TO_REGISTER.last_mut().unwrap();

        match event_name.into_str() {
            other => {
                plugin
                    .1
                    .arbitrary_subscribed_events
                    .insert(other.to_owned());
            }
        }
    }

    unsafe extern "C" fn register_subplugin(
        subplugin_name: BwsStr,
    ) -> Tuple3<SubPluginEventReceiver, SubPluginEntry, SubPluginSubscribeToEvent> {
        let plugin = TO_REGISTER.last_mut().unwrap();

        let subplugin_name = subplugin_name.into_str().to_owned();

        let (gate, receiver) = Gate::new();

        plugin.1.subplugins.push(SubPlugin {
            name: subplugin_name,
            gate,
            subscribed_events: [0; (SubPluginEvent::VARIANT_COUNT + 7) / 8],
            arbitrary_subscribed_events: HashSet::new(),
            entry: None,
        });

        Tuple3(
            SubPluginEventReceiver((receiver as *const _ as *const ()).as_ref().unwrap()),
            subplugin_entry,
            subplugin_subscribe_to_event,
        )
    }

    unsafe extern "C" fn subplugin_entry(future: FfiFuture<Unit>) {
        let plugin = TO_REGISTER.last_mut().unwrap();
        let subplugin = plugin.1.subplugins.last_mut().unwrap();

        subplugin.entry = Some(future);
    }

    unsafe extern "C" fn subplugin_subscribe_to_event(event_name: BwsStr) {
        let plugin = TO_REGISTER.last_mut().unwrap();
        let subplugin = plugin.1.subplugins.last_mut().unwrap();

        let event_name = event_name.into_str();

        match event_name {
            other => {
                subplugin
                    .arbitrary_subscribed_events
                    .insert(other.to_owned());
            }
        }
    }

    let plugin_registrator: Symbol<unsafe extern "C" fn(RegisterPlugin)> =
        lib.get(b"bws_load_library")?;

    (*plugin_registrator)(register_plugin);

    let mut to_register_non_static = Vec::new();
    swap(&mut TO_REGISTER, &mut to_register_non_static);
    for plugin in to_register_non_static {
        plugins.insert(plugin.0, plugin.1);
    }

    Ok(())
}

fn check_dependencies(plugin_name: &str, plugins: &Plugins) -> Result<bool> {
    let mut result = true;

    let plugin = &plugins[plugin_name];
    for dependency in &plugin.dependencies {
        let dependency_req =
            VersionReq::parse(&dependency.1).context("error parsing version requirement")?;

        match plugins.get(&dependency.0) {
            Some(dep_plugin) => {
                if !dependency_req.matches(&dep_plugin.version) {
                    error!(
                        "Plugin {:?} dependency {:?} {} was not met. {:?} {} is present, but does not match the {} version requirement.",
                        plugin_name, dependency.0, dependency_req, dependency.0, dep_plugin.version, dependency_req
                    );
                    result = false;
                }
            }
            None => {
                error!(
                    "Plugin {:?} dependency {:?} {} was not met.",
                    plugin_name, dependency.0, dependency_req
                );
                result = false;
            }
        }
    }
    Ok(result)
}
