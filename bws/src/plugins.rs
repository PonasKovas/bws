use anyhow::{bail, Context, Result};
use async_ffi::FfiFuture;
use bws_plugin::*;
use libloading::{Library, Symbol};
use log::{error, info};
use semver::{Version, VersionReq};
use sha2::digest::generic_array::transmute;
use std::mem::swap;
use std::path::PathBuf;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    path::Path,
    sync::Arc,
};
use tokio::fs;

use crate::plugins;

const ABI_VERSION: u64 = ((async_ffi::ABI_VERSION as u64) << 32) | crate::ABI_VERSION as u64;

pub type Plugins = HashMap<String, Plugin>;
pub struct Plugin {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, String)>,
    pub callbacks: Callbacks,
    pub subplugins: Vec<SubPlugin>,
}
pub struct SubPlugin {
    pub name: String,
    pub callbacks: SubPluginCallbacks,
}

#[derive(Default)]
pub struct Callbacks {
    init: Option<extern "C" fn() -> FfiFuture<()>>,
    /// Other callbacks the plugin may register, that may be used by other plugins
    /// It will be up to them transmute the pointer to a correct function pointer
    other: HashMap<String, *const ()>,
}

#[derive(Default)]
pub struct SubPluginCallbacks {
    init: Option<extern "C" fn() -> FfiFuture<()>>,
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

    for plugin in &plugins {
        info!(
            "Plugin {:?} loaded. (Provided by {:?})",
            plugin.0, plugin.1.provided_by
        );
    }

    Ok(plugins)
}

async unsafe fn load_library(plugins: &mut Plugins, path: impl AsRef<Path>) -> Result<()> {
    let lib = Arc::new(Library::new(path.as_ref())?);

    let abi_version: Symbol<*const u64> = lib.get(b"BWS_ABI_VERSION")?;

    if **abi_version != ABI_VERSION {
        bail!(
        	"plugin is compiled with a non-compatible ABI version. BWS uses {}, while the library was compiled with {}.",
        	ABI_VERSION,
        	**abi_version
        );
    }

    // register the plugins

    // Vec<((plugin_name, version), dependencies, callbacks, subplugins)>
    #[allow(non_upper_case_globals)]
    static mut to_register: Vec<(
        (String, String),
        Vec<(String, String)>,
        Callbacks,
        Vec<SubPlugin>,
    )> = Vec::new();

    unsafe extern "C" fn register_plugin(
        name: BwsStr,
        version: BwsStr,
        dependencies: BwsSlice,
    ) -> Tuple2<RegisterCallback, RegisterSubPlugin> {
        to_register.push((
            (name.into_str().to_owned(), version.into_str().to_owned()),
            dependencies
                .into_slice::<Tuple2<BwsStr, BwsStr>>()
                .iter()
                .map(|e| (e.0.into_str().to_owned(), e.1.into_str().to_owned()))
                .collect(),
            Default::default(),
            Vec::new(),
        ));

        Tuple2(register_callback, register_subplugin)
    }

    unsafe extern "C" fn register_callback(callback_name: BwsStr, fn_ptr: *const ()) {
        let plugin = to_register.last_mut().unwrap();

        match callback_name.into_str() {
            "init" => {
                plugin.2.init = Some(transmute(fn_ptr));
            }
            other => {
                plugin.2.other.insert(other.to_owned(), fn_ptr);
            }
        }
    }

    unsafe extern "C" fn register_subplugin(subplugin_name: BwsStr) -> RegisterSubPluginCallback {
        let plugin = to_register.last_mut().unwrap();

        let subplugin_name = subplugin_name.into_str().to_owned();

        plugin.3.push(SubPlugin {
            name: subplugin_name,
            callbacks: Default::default(),
        });

        register_subplugin_callback
    }

    unsafe extern "C" fn register_subplugin_callback(callback_name: BwsStr, fn_ptr: *const ()) {
        let plugin = to_register.last_mut().unwrap();
        let subplugin = plugin.3.last_mut().unwrap();

        let callback_name = callback_name.into_str();

        match callback_name {
            "init" => {
                subplugin.callbacks.init = Some(transmute(fn_ptr));
            }
            _ => {
                error!(
                    "Plugin {:?} subplugin {:?} tried to register an unknown callback: {:?}",
                    plugin.0 .0, subplugin.name, callback_name
                );
                return;
            }
        }
    }

    let plugin_registrator: Symbol<unsafe extern "C" fn(RegisterPlugin)> =
        lib.get(b"bws_load_library")?;

    (*plugin_registrator)(register_plugin);

    let mut to_register_non_static = Vec::new();
    swap(&mut to_register, &mut to_register_non_static);
    for plugin in to_register_non_static {
        plugins.insert(
            plugin.0 .0,
            Plugin {
                version: Version::parse(&plugin.0 .1)
                    .context("Unable to parse plugin's version")?,
                provided_by: path.as_ref().to_path_buf(),
                dependencies: plugin.1,
                callbacks: plugin.2,
                subplugins: plugin.3,
            },
        );
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
