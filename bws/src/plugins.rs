use anyhow::{bail, Context, Result};
use async_ffi::{FfiContext, FfiFuture, FfiPoll};
use bws_plugin::prelude::*;
use bws_plugin::PluginEntrySignature;
use libloading::{Library, Symbol};
use log::{error, info};
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
use tokio::sync::oneshot;
use tokio::sync::RwLock;

const ABI_VERSION: u16 = 0;
const BWS_ABI_VERSION: u64 = ((async_ffi::ABI_VERSION as u64) << 32)
    | ((bws_plugin::BWS_PLUGIN_ABI_VERSION as u64) << 16)
    | ABI_VERSION as u64;

pub struct Plugin {
    name: String,
    gate: Option<Gate>, // None if the plugin is not active
    plugin: PluginData,
}

pub struct PluginData {
    pub version: Version,
    pub provided_by: PathBuf,
    pub dependencies: Vec<(String, VersionReq)>,
    pub library: Arc<Library>,
    pub entry: PluginEntrySignature,
}

pub struct Gate {
    // sender: mpsc::UnboundedSender<BwsTuple2<BwsEvent<'static>, SendPtr<oneshot::Sender<()>>>>,
}

pub async fn load_plugins() -> Result<()> {
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

    // // check if all dependencies of plugins are satisfied
    // for plugin in &plugins {
    //     match check_dependencies(plugin.0, &plugins) {
    //         Ok(true) => {}
    //         Ok(false) => {
    //             bail!(
    //                 "Plugin {:?} could not be loaded, because it's dependencies weren't satisfied. (Provided by {:?})",
    //                 plugin.0, plugins[plugin.0].plugin.provided_by
    //             );
    //         }
    //         Err(e) => {
    //             bail!(
    //                 "Error reading dependencies of plugin {:?} (Provided by {:?}): {:?}",
    //                 plugin.0,
    //                 plugins[plugin.0].plugin.provided_by,
    //                 e
    //             );
    //         }
    //     }
    // }

    Ok(())
}

async unsafe fn load_library(plugins: &mut Vec<Plugin>, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let lib = Arc::new(Library::new(path)?);

    let abi_version: Symbol<*const u64> = lib.get(b"BWS_ABI_VERSION")?;

    if **abi_version != BWS_ABI_VERSION {
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
    static mut registered: Vec<PluginStructure> = Vec::new();

    #[repr(C)]
    struct PluginStructure {
        name: BwsStr<'static>,
        version: BwsTuple3<u64, u64, u64>,
        dependencies: BwsSlice<'static, BwsTuple2<BwsStr<'static>, BwsStr<'static>>>,
        entry: PluginEntrySignature,
    }

    unsafe extern "C" fn register(plugin: PluginStructure) {
        registered.push(plugin);
    }

    let init: Symbol<unsafe extern "C" fn(unsafe extern "C" fn(PluginStructure))> =
        lib.get(b"bws_library_init")?;

    (*init)(register);

    // now parse the contents of the static variable

    for plugin in &registered {
        // make sure the plugin name is unique
        if let Some(other_plugin) = plugins
            .iter()
            .find(|other_plugin| other_plugin.name == plugin.name.as_str())
        {
            error!(
                "Plugin name collision: {:?} both registered by {:?} and {:?}",
                plugin.name.as_str(),
                path,
                other_plugin.plugin.provided_by
            );
            continue;
        }

        plugins.push(Plugin {
            name: plugin.name.as_str().to_owned(),
            gate: None,
            plugin: PluginData {
                version: Version::new(plugin.version.0, plugin.version.1, plugin.version.2),
                provided_by: path.to_path_buf(),
                dependencies: plugin
                    .dependencies
                    .as_slice()
                    .into_iter()
                    .try_fold::<_, _, Result<_>>(Vec::new(), |mut acc, dep| {
                        acc.push((
                            dep.0.as_str().to_owned(),
                            VersionReq::parse(dep.1.as_str())
                                .context("error parsing version requirement")?,
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
