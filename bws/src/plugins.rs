use anyhow::{bail, Context, Result};
use async_ffi::FfiFuture;
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

pub type Plugins = HashMap<CString, Plugin>;
pub struct Plugin {
    pub version: CString,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(CString, CString)>,
    pub callbacks: Callbacks,
    pub subplugins: Vec<SubPlugin>,
}
pub struct SubPlugin {
    pub name: CString,
    pub callbacks: SubPluginCallbacks,
}

#[derive(Default)]
pub struct Callbacks {
    init: Option<Callback<extern "C" fn(), extern "C" fn() -> FfiFuture<()>>>,
    /// Other callbacks the plugin may register, that may be used by other plugins
    /// It will be up to them transmute the pointer to a correct function pointer
    other: HashMap<String, Callback<*const (), *const ()>>,
}

#[derive(Default)]
pub struct SubPluginCallbacks {
    init: Option<Callback<extern "C" fn(), extern "C" fn() -> FfiFuture<()>>>,
}

#[repr(C)]
pub enum Callback<S, A> {
    Sync(S),
    Async(A),
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

    let abi_version: Symbol<*const u64> = lib.get(b"ABI_VERSION")?;

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
        (CString, CString),
        Vec<(CString, CString)>,
        Callbacks,
        Vec<SubPlugin>,
    )> = Vec::new();

    type RegisterPluginType =
        unsafe extern "C" fn(
            *const i8,
            *const i8,
            usize,
            *const *const i8,
        ) -> Tuple2<RegisterCallbackType, RegisterSubPluginType>;
    unsafe extern "C" fn register_plugin(
        name: *const i8,
        version: *const i8,
        dependencies_n: usize,
        dependencies: *const *const i8,
    ) -> Tuple2<RegisterCallbackType, RegisterSubPluginType> {
        to_register.push((
            (
                CStr::from_ptr(name).to_owned(),
                CStr::from_ptr(version).to_owned(),
            ),
            {
                let mut deps = Vec::new();
                for i in 0..dependencies_n {
                    deps.push((
                        CStr::from_ptr(*dependencies.add(i * 2)).to_owned(),
                        CStr::from_ptr(*dependencies.add(i * 2 + 1)).to_owned(),
                    ));
                }
                deps
            },
            Default::default(),
            Vec::new(),
        ));

        Tuple2(register_callback, register_subplugin)
    }

    type RegisterCallbackType = unsafe extern "C" fn(*const i8, *const (), bool);
    unsafe extern "C" fn register_callback(
        callback_name: *const i8,
        fn_ptr: *const (),
        is_async: bool,
    ) {
        let plugin = to_register.last_mut().unwrap();

        let callback_name = CStr::from_ptr(callback_name);
        let callback_name = match callback_name.to_str() {
            Ok(s) => s,
            Err(_) => {
                error!(
                    "Plugin {:?} tried to register a callback with a name that is not valid unicode: {:?}",
                    plugin.0.0,
                    callback_name
                );
                return;
            }
        };

        match callback_name {
            "init" => {
                if is_async {
                    plugin.2.init = Some(Callback::Async(transmute(fn_ptr)));
                } else {
                    plugin.2.init = Some(Callback::Sync(transmute(fn_ptr)));
                }
            }
            other => {
                if is_async {
                    plugin
                        .2
                        .other
                        .insert(other.to_owned(), Callback::Async(fn_ptr));
                } else {
                    plugin
                        .2
                        .other
                        .insert(other.to_owned(), Callback::Sync(fn_ptr));
                }
            }
        }
    }

    type RegisterSubPluginType = unsafe extern "C" fn(*const i8) -> RegisterSubPluginCallbackType;
    unsafe extern "C" fn register_subplugin(
        subplugin_name: *const i8,
    ) -> RegisterSubPluginCallbackType {
        let plugin = to_register.last_mut().unwrap();

        let subplugin_name = CStr::from_ptr(subplugin_name);

        plugin.3.push(SubPlugin {
            name: subplugin_name.to_owned(),
            callbacks: Default::default(),
        });

        register_subplugin_callback
    }

    type RegisterSubPluginCallbackType = unsafe extern "C" fn(*const i8, *const (), bool);
    unsafe extern "C" fn register_subplugin_callback(
        callback_name: *const i8,
        fn_ptr: *const (),
        is_async: bool,
    ) {
        let plugin = to_register.last_mut().unwrap();
        let subplugin = plugin.3.last_mut().unwrap();

        let callback_name = CStr::from_ptr(callback_name);
        let callback_name = match callback_name.to_str() {
            Ok(s) => s,
            Err(_) => {
                error!(
                    "Plugin {:?} subplugin {:?} tried to register a callback with a name that is not valid unicode: {:?}",
                    plugin.0.0,
                    subplugin.name,
                    callback_name
                );
                return;
            }
        };

        match callback_name {
            "init" => {
                if is_async {
                    subplugin.callbacks.init = Some(Callback::Async(transmute(fn_ptr)));
                } else {
                    subplugin.callbacks.init = Some(Callback::Sync(transmute(fn_ptr)));
                }
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

    let plugin_registrator: Symbol<unsafe extern "C" fn(RegisterPluginType)> =
        lib.get(b"register_plugin")?;

    (*plugin_registrator)(register_plugin);

    let mut to_register_non_static = Vec::new();
    swap(&mut to_register, &mut to_register_non_static);
    for plugin in to_register_non_static {
        plugins.insert(
            plugin.0 .0,
            Plugin {
                version: plugin.0 .1,
                provided_by: path.as_ref().to_path_buf(),
                dependencies: plugin.1,
                callbacks: plugin.2,
                subplugins: plugin.3,
            },
        );
    }

    Ok(())
}

fn check_dependencies(plugin_name: &CString, plugins: &Plugins) -> Result<bool> {
    let mut result = true;

    let plugin = &plugins[plugin_name];
    for dependency in &plugin.dependencies {
        let dependency_req = VersionReq::parse(dependency.1.to_str()?)
            .context("error parsing version requirement")?;

        match plugins.get(&dependency.0) {
            Some(dep_plugin) => {
                let dep_version = Version::parse(dep_plugin.version.to_str()?)?;
                if !dependency_req.matches(&dep_version) {
                    error!(
                        "Plugin {:?} dependency {:?} {} was not met. {:?} {} is present, but does not match the {} version requirement.",
                        plugin_name, dependency.0, dependency_req, dependency.0, dep_version, dependency_req
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

#[repr(C)]
struct Tuple2<T1: Sized, T2>(T1, T2);
