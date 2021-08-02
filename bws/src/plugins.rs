use anyhow::{bail, Context, Result};
use async_ffi::FfiFuture;
use libloading::{Library, Symbol};
use log::{error, info};
use semver::{Version, VersionReq};
use std::{collections::HashMap, path::Path};
use tokio::fs;

const ABI_VERSION: u32 = async_ffi::ABI_VERSION << 16 | crate::ABI_VERSION;

pub struct Plugins {}

pub async fn load_plugins() -> Result<Plugins> {
    let mut plugins = HashMap::new();

    let mut read_dir = fs::read_dir("plugins").await?;
    while let Some(path) = read_dir.next_entry().await? {
        let path = path.path().canonicalize()?;

        // ignore directories and files starting with .
        if path.is_dir() {
            continue;
        }
        match path.file_name().unwrap().to_str() {
            // also skip files with invalid unicode in their names
            None => continue,
            Some(path) => {
                if path.starts_with('.') {
                    continue;
                }
            }
        }

        match unsafe { load_plugin(&path).await } {
            Ok((name, version, lib)) => {
                plugins.insert(name, (version, lib));
            }
            Err(e) => {
                error!("Error loading {:?}: {:?}", path.file_name().unwrap(), e);
            }
        }
    }

    // ok, the libraries have been loaded, now time to check their dependencies, and if met, initialize them
    for plugin in &plugins {
        match unsafe { dependencies_matched(plugin, &plugins) } {
            Ok(true) => {
                // everything alright, now we can finally initialize and prepare them for usage
            }
            Ok(false) => {
                // :/
                // sad, but gotta move on
            }
            Err(e) => {
                error!("Error reading dependencies of plugin {}: {:?}", plugin.0, e);
                // now this is truly sad! but still have to move on...
            }
        }
    }

    todo!()
}

async unsafe fn load_plugin(path: impl AsRef<Path>) -> Result<(String, Version, Library)> {
    let lib = Library::new(path.as_ref())?;

    let abi_version: Symbol<*const u32> = lib.get(b"ABI_VERSION")?;

    if **abi_version != ABI_VERSION {
        bail!(
        	"plugin is compiled with a non-compatible ABI version. BWS uses {}, while the library was compiled with {}.",
        	ABI_VERSION,
        	**abi_version
        );
    }

    let plugin_name: Symbol<*const &[u8]> = lib.get(b"PLUGIN_NAME")?;
    let plugin_version: Symbol<*const &[u8]> = lib.get(b"PLUGIN_VERSION")?;

    Ok((
        std::str::from_utf8(**plugin_name)
            .context("reading PLUGIN_NAME")?
            .to_owned(),
        Version::parse(std::str::from_utf8(**plugin_version).context("reading PLUGIN_VERSION")?)
            .context("parsing PLUGIN_VERSION")?,
        lib,
    ))
}

unsafe fn dependencies_matched(
    plugin: (&String, &(Version, Library)),
    plugins: &HashMap<String, (Version, Library)>,
) -> Result<bool> {
    let dependencies: Symbol<*const &[(&[u8], &[u8])]> =
        (plugin.1).1.get(b"PLUGIN_DEPENDENCIES")?;

    for dependency in **dependencies {
        let dependency_name = std::str::from_utf8(dependency.0)?;
        let dependency_req = std::str::from_utf8(dependency.1)?;

        let dependency_req = semver::VersionReq::parse(dependency_req)
            .context("error parsing version requirement")?;

        match plugins.get(dependency_name) {
            Some(dep_plugin) => {
                if !dependency_req.matches(&dep_plugin.0) {
                    error!(
                        "Plugin's \"{}\" dependency {} {} was not met. {} {} is present, but does not match the {} version requirement. Skipping.",
                        plugin.0, dependency_name, dependency_req, dependency_name, dep_plugin.0, dependency_req
                    );
                    return Ok(false);
                }
            }
            None => {
                error!(
                    "Plugin's \"{}\" dependency {} {} was not met. Skipping.",
                    plugin.0, dependency_name, dependency_req
                );
                return Ok(false);
            }
        }
    }

    Ok(true)
}
