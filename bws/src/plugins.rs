use crate::LinearSearch;
use anyhow::{bail, Context, Result};
use bws_plugin_interface::safe_types::*;
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use log::{error, info, warn};
use semver::{Version, VersionReq};
use std::collections::HashSet;
use std::fmt::Debug;
use std::thread;
use std::time::Duration;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const PLUGIN_DIR: &str = "plugins/";

pub struct PluginData {
    file_path: PathBuf,
    plugin: &'static BwsPlugin,
    raw_library: &'static Library,
}

pub fn load_plugins() -> Result<Vec<PluginData>> {
    let mut libs = Vec::new();

    let mut success = true;

    for entry in fs::read_dir(PLUGIN_DIR)? {
        let path = entry?.path();

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
        match unsafe { load_lib(&path) } {
            Ok(l) => libs.push(l),
            Err(e) => {
                error!("Error loading {:?}: {e:?}", path.file_name().unwrap());
                success = false;
            }
        }
    }

    // Make sure all plugins have unique names
    let mut names = HashSet::new();
    let mut names_with_collisions = Vec::new();
    for lib in &libs {
        if !names.insert(lib.plugin.name) {
            names_with_collisions.push(lib.plugin.name);
            success = false;
        }
    }

    for name in &names_with_collisions {
        error!(
            "Plugin name collision: {:?} is provided by the following libraries:",
            name
        );
        // list the libraries that provide plugins with the name thats causing trouble
        for lib in &libs {
            if lib.plugin.name == *name {
                error!(" - {} ({})", lib.file_path.display(), lib.plugin.version);
            }
        }
    }

    // make sure each plugin has a valid version
    for lib in &libs {
        if let Err(e) = Version::parse(lib.plugin.version.as_str()) {
            error!(
                "Plugin {:?} ({}) version {:?} could not be parsed: {}",
                lib.plugin.name,
                lib.file_path.display(),
                lib.plugin.version,
                e
            );
            success = false;
        }
    }

    // if any problems encountered up until this point, we can already return since checking for dependencies is useless
    // if the plugins cant even say their name or version right
    if !success {
        bail!("Some plugins couldn't be loaded. You have to resolve the reported errors before you can launch BWS.");
    }

    // Check if dependencies are satisfied
    for lib in 0..libs.len() {
        if !check_dependencies(&libs, lib).context("Error checking dependencies")? {
            error!(
                "Dependencies of {} {} ({}) are not satisfied so it couldn't be loaded.",
                libs[lib].plugin.name,
                libs[lib].plugin.version,
                libs[lib].file_path.display()
            );
            success = false;
        }
    }

    if success {
        Ok(libs)
    } else {
        bail!("Some plugins couldn't be loaded. You have to resolve the reported errors before you can launch BWS.");
    }
}

pub unsafe fn load_lib(path: impl AsRef<Path>) -> Result<PluginData> {
    let path = path.as_ref();

    let lib = unsafe { Library::new(path)? };
    let abi: Symbol<*const u64> = unsafe {
        lib.get(b"BWS_ABI")
            .context("Error getting BWS_ABI symbol in plugin")?
    };

    if unsafe { **abi } != bws_plugin_interface::ABI {
        bail!(
            "ABI is incompatible. BWS uses {}, and the plugin uses {}",
            bws_plugin_interface::ABI,
            unsafe { **abi }
        );
    }

    let root = *unsafe {
        lib.get::<*const BwsPlugin>(b"BWS_PLUGIN_ROOT")
            .context("BWS_PLUGIN_ROOT not found")?
    };

    Ok(PluginData {
        file_path: path.to_path_buf(),
        plugin: unsafe { root.as_ref().unwrap() },
        raw_library: Box::leak(Box::new(lib)),
    })
}

pub fn check_dependencies(libs: &[PluginData], lib: usize) -> Result<bool> {
    let mut res = true;

    let deps = libs[lib].plugin.dependencies.as_slice();
    for dep in deps {
        let dep_name = dep.0.as_str();
        let version_req =
            VersionReq::parse(dep.1.as_str()).context("Couldn't parse version requirement")?;

        // first check if a plugin with the name exists
        match libs
            .iter()
            .find(|plugin| plugin.plugin.name.as_str() == dep_name)
        {
            Some(m) => {
                // Check if version matches
                let version =
                    Version::parse(m.plugin.version.as_str()).context("Couldn't parse version")?;

                if !version_req.matches(&version) {
                    error!(
                        "{}: needs {dep_name} {version_req} which wasn't found. {dep_name} {version} found, but versions incompatible.",
                        libs[lib].plugin.name
                    );
                    res = false;
                }
            }
            None => {
                error!(
                    "{}: needs {dep_name} {version_req} which wasn't found.",
                    libs[lib].plugin.name,
                );
                res = false;
            }
        }
    }

    Ok(res)
}

pub fn init_plugins(plugins: &Vec<PluginData>) -> Result<()> {
    // Use the graph theory to order the plugins so that they would load
    // only after all of their dependencies have loaded.

    let mut graph = petgraph::graph::DiGraph::<SStr<'static>, ()>::new();
    let mut indices: Vec<(SStr<'static>, _)> = Vec::new();
    for plugin in plugins {
        indices.push((plugin.plugin.name, graph.add_node(plugin.plugin.name)));
    }

    // set the edges
    // (in other words, connect dependencies)
    for plugin in plugins {
        let id = indices.search(&plugin.plugin.name);
        for dependency in plugin.plugin.dependencies {
            graph.update_edge(*indices.search(&dependency.0), *id, ());
        }
    }

    // perform a topological sort of the nodes 😎
    let ordering = match petgraph::algo::toposort(&graph, None) {
        Ok(o) => o,
        Err(cycle) => {
            bail!(
                "Dependency cycle detected: {}",
                indices.search_by_val(&cycle.node_id())
            );
        }
    };

    // now that we know the order, we can start the plugins one by one
    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        for plugin in plugins {
            if plugin.plugin.name == *plugin_name {
                (plugin.plugin.on_load)(&crate::vtable::VTABLE);
                break;
            }
        }
    }

    Ok(())
}
