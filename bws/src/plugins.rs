mod vtable;

use crate::LinearSearch;
use anyhow::{bail, Context, Result};
use async_ffi::{FfiContext, FfiFuture, FfiPoll, LocalFfiFuture};
use bws_plugin::register::{PluginEntrySignature, RegPluginStruct};
use bws_plugin::{prelude::*, LogLevel};
use libloading::{Library, Symbol};
use log::{debug, error, info, trace, warn};
use semver::{Version, VersionReq};
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
use tokio::sync::oneshot::{self, channel};
use tokio::sync::RwLock;

const BWS_ABI_VERSION: u64 =
    ((async_ffi::ABI_VERSION as u64) << 32) | (bws_plugin::ABI_VERSION as u64);

pub struct Plugin {
    name: String,
    // None if the plugin is not active at the moment
    /// Event id, event data pointer (optional, depending on event), oneshot sender pointer
    event_sender: Option<mpsc::UnboundedSender<BwsTuple3<u32, SendPtr<()>, SendPtr<()>>>>,
    plugin_data: PluginData,
}

pub struct PluginData {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, VersionReq)>,
    pub library: Arc<Library>,
    pub entry: PluginEntrySignature,
}

pub async fn load_plugins() -> Result<Vec<Plugin>> {
    let mut plugins: Vec<Plugin> = Vec::new();

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
        if !check_dependencies(&plugin, &plugins) {
            bail!(
                "Plugin {:?} could not be loaded, because it's dependencies weren't satisfied. (Provided by {:?})",
                plugin.name, plugin.plugin_data.provided_by
            );
        }
    }

    Ok(plugins)
}

// loads a library file and adds the plugins it registers
async unsafe fn load_library(plugins: &mut Vec<Plugin>, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let lib = Arc::new(unsafe { Library::new(path)? });

    let abi_version: Symbol<*const u64> = unsafe { lib.get(b"BWS_ABI_VERSION")? };

    if unsafe { **abi_version } != BWS_ABI_VERSION {
        bail!(
        	"plugin is compiled with a non-compatible ABI version. BWS uses {}, while the library was compiled with {}.",
        	BWS_ABI_VERSION,
        	**abi_version
        );
    }

    // register the plugins

    // this is the thing that FFI functions will operate on and
    // that's why it has to be static.
    // later it's content will be parsed and moved to the plugins variable.
    #[allow(non_upper_case_globals)]
    static mut registered: Vec<RegPluginStruct> = Vec::new();

    unsafe extern "C" fn register(plugin: RegPluginStruct) {
        unsafe { registered.push(plugin) }
    }

    let init: Symbol<unsafe extern "C" fn(unsafe extern "C" fn(RegPluginStruct))> =
        unsafe { lib.get(b"bws_library_init")? };

    unsafe { (*init)(register) };

    // now parse the contents of the static variable

    // can't move out of a static so efficiently make the vector non-static
    // and leave a new empty vector in it's place
    let mut non_static_registered = Vec::new();
    std::mem::swap(&mut non_static_registered, unsafe { &mut registered });

    for plugin in non_static_registered {
        // make sure the plugin name is unique
        if let Some(other_plugin) = plugins
            .iter()
            .find(|other_plugin| other_plugin.name == plugin.name.as_bws_str().as_str())
        {
            error!(
                "Plugin name collision: {:?} both registered by {:?} and {:?}",
                plugin.name.into_string(),
                path,
                other_plugin.plugin_data.provided_by
            );
            continue;
        }

        plugins.push(Plugin {
            name: plugin.name.into_string(),
            event_sender: None,
            plugin_data: PluginData {
                version: Version::new(plugin.version.0, plugin.version.1, plugin.version.2),
                provided_by: path.to_path_buf(),
                dependencies: plugin
                    .dependencies
                    .into_vec()
                    .into_iter()
                    .try_fold::<_, _, Result<_>>(Vec::new(), |mut acc, dep| {
                        let name = dep.0.into_string();
                        let version_req = dep.1.into_string();
                        acc.push((
                            name,
                            VersionReq::parse(&version_req).with_context(|| {
                                format!("error parsing version requirement {:?}", version_req)
                            })?,
                        ));

                        Ok(acc)
                    })?,
                entry: plugin.entry,
                library: Arc::clone(&lib),
            },
        });
    }

    Ok(())
}

// checks if all dependencies of the given plugin are satisfied
fn check_dependencies(plugin: &Plugin, plugins: &Vec<Plugin>) -> bool {
    let mut result = true;

    for dependency in &plugin.plugin_data.dependencies {
        if let Some(dep_plugin) = plugins.iter().find(|p| p.name == dependency.0) {
            if !dependency.1.matches(&dep_plugin.plugin_data.version) {
                error!(
                        "Plugin {:?} depends on {:?} {} which was not found. {:?} {} is present, but does not match the {} version requirement.",
                        plugin.name, dependency.0, dependency.1, dependency.0, dep_plugin.plugin_data.version, dependency.1
                    );
                result = false;
            }
        } else {
            error!(
                "Plugin {:?} depends on {:?} {} which was not found.",
                plugin.name, dependency.0, dependency.1
            );
            result = false;
        }
    }
    result
}

pub async fn start_plugins(plugins: &mut Vec<Plugin>) -> Result<()> {
    // Use the graph theory to order the plugins so that they would load
    // only after all of their dependencies have loaded.
    let mut graph = petgraph::graph::DiGraph::<String, ()>::new();
    let mut indices = Vec::new();
    for plugin in &*plugins {
        indices.push((plugin.name.clone(), graph.add_node(plugin.name.clone())));
    }

    // set the edges
    // (in other words, connect the nodes with arrows in the way of dependence)
    for plugin in &*plugins {
        let id = indices.search(&plugin.name);
        for dependency in &plugin.plugin_data.dependencies {
            graph.update_edge(*indices.search(&dependency.0), *id, ());
        }
    }

    // perform the topological sort of the nodes 😎
    let ordering = match petgraph::algo::toposort(&graph, None) {
        Ok(o) => o,
        Err(cycle) => {
            bail!(
                "Dependency cycle detected: {}",
                indices.search_by_val(&cycle.node_id())
            );
        }
    };

    // now that we now the order, we can start the plugins one by one
    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        let plugin_id = plugins.iter().position(|p| &p.name == plugin_name).unwrap();
        let plugin = &mut plugins[plugin_id];

        // Create the events channel
        let (sender, receiver) = mpsc::unbounded_channel();
        let receiver = SendPtr(Box::leak(Box::new(receiver)) as *const _ as *const ());

        tokio::spawn(unsafe {
            (plugin.plugin_data.entry)(
                BwsString::from_string(plugin_name.clone()),
                vtable::VTABLE.clone(),
                receiver,
            )
        });

        plugin.event_sender = Some(sender);

        info!(
            "Plugin {:?} loaded and started. (Provided by {:?})",
            plugin_name, plugin.plugin_data.provided_by
        );

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let (oneshot_sender, oneshot_receiver) = channel::<()>();
        let oneshot_sender = ManuallyDrop::new(oneshot_sender);
        plugin
            .event_sender
            .as_ref()
            .unwrap()
            .send(BwsTuple3(
                14,
                SendPtr(null()),
                SendPtr(&*oneshot_sender as *const _ as *const ()),
            ))
            .unwrap();
        info!("{:?}", oneshot_receiver.await);
    }

    Ok(())
}

// defined here instead of vtable, so that the logger says bws::plugins
// and not bws::plugins::vtable
fn plugin_log(msg: BwsStr<'static>, level: LogLevel) {
    match level {
        LogLevel::Error => {
            error!("{}", msg.as_str());
        }
        LogLevel::Warning => {
            warn!("{}", msg.as_str());
        }
        LogLevel::Info => {
            info!("{}", msg.as_str());
        }
        LogLevel::Debug => {
            debug!("{}", msg.as_str());
        }
        LogLevel::Trace => {
            trace!("{}", msg.as_str());
        }
    }
}
