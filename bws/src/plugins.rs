mod vtable;

use anyhow::{bail, Context, Result};
use async_ffi::{FfiContext, FfiFuture, FfiPoll};
use bws_plugin::pointers::global_state::BwsGlobalState;
use bws_plugin::prelude::*;
use bws_plugin::register::{_f_PluginEntry, _f_SubPluginEntry};
use bws_plugin::vtable::BwsVTable;
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
use bitvec::prelude::*;

const ABI_VERSION: u64 = ((async_ffi::ABI_VERSION as u64) << 32) | crate::ABI_VERSION as u64;

pub struct Plugin {
    gate: Option<Gate>, // None if the plugin is not active
    plugin: PluginData,
}

pub struct PluginData {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, VersionReq)>,
    /// Bitmask of event IDs
    pub subscribed_events: BitVec,
    pub library: Arc<Library>,
    pub entry: _f_PluginEntry,
}

struct CandidatePlugin {
    name: String,
    version: Version,
    dependencies: Vec<(String, String)>,
    entry: _f_PluginEntry,
    subplugins: Vec<SubPluginData>,
}

struct SubPluginData {
    name: String,
    /// Bitmask of event IDs
    subscribed_events: BitVec,
    entry: _f_SubPluginEntry,
}

pub struct Gate {
    sender: mpsc::UnboundedSender<BwsTuple2<BwsEvent<'static>, SendPtr<oneshot::Sender<()>>>>,
}

impl Gate {
    pub fn new() -> (
        Self,
        &'static mut mpsc::UnboundedReceiver<
            BwsTuple2<BwsEvent<'static>, SendPtr<oneshot::Sender<()>>>,
        >,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();

        (Self { sender }, Box::leak(Box::new(receiver)))
    }
    // If None, the plugin died while handling the call or was already dead
    pub async fn call(&self, message: BwsEvent<'static>) -> Option<()> {
        let (sender, receiver) = oneshot::channel();
        let sender = ManuallyDrop::new(sender);
        if let Err(_) = self
            .sender
            .send(BwsTuple2(message, SendPtr(&*sender as *const _)))
        {
            return None;
        }
        receiver.await.ok()
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
                bws_plugin::pointers::plugin_gate::BwsPluginGate::new(
                    receiver as *const _ as *const (),
                ),
                BwsGlobalState::new(Arc::into_raw(Arc::clone(global_state)) as *const ()),
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
    let event = BwsEvent::new(0, &mut response as *mut _ as *mut ());
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

// Returns plugins and event name mappings
pub async fn load_plugins() -> Result<(HashMap<String, Plugin>, Box<Vec<String>>)> {
    let mut plugins: HashMap<String, Plugin> = HashMap::new();
    let mut events: Box<Vec<String>> = Box::new(Vec::new());

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

        if let Err(e) = unsafe { load_library(&mut plugins, &mut events, &path).await } {
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

    Ok((plugins, events))
}

async unsafe fn load_library(
    plugins: &mut HashMap<String, Plugin>,
    events: &mut Box<Vec<String>>,
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

    static mut registered: Vec<CandidatePlugin> = Vec::new();

    #[repr(C)]
    struct PluginStructure {
        name: BwsStr<'static>,
        version: BwsStr<'static>,
        dependencies: BwsSlice<'static, BwsTuple2<BwsStr<'static>, BwsStr<'static>>>,
        entry: _f_PluginEntry,
        subplugins: BwsSlice<'static, SubPluginStructure>,
    }

    #[repr(C)]
    struct SubPluginStructure {
        name: BwsStr<'static>,
        dependencies: BwsSlice<'static, BwsTuple2<BwsStr<'static>, BwsStr<'static>>>,
        entry: _f_SubPluginEntry,
    }

    unsafe extern "C" fn register(plugin: PluginStructure) {}

    let mut to_register: Vec<CandidatePlugin> = Vec::new();

    let init: Symbol<unsafe extern "C" fn(*mut (), &BwsVTable)> = lib.get(b"bws_library_init")?;

    (*init)(&mut to_register as *mut _ as *mut (), &vtable::VTABLE);

    for plugin in to_register {
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
