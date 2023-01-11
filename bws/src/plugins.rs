use crate::LinearSearch;
use anyhow::{bail, Context, Result};
use bws_plugin_interface::safe_types::*;
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use log::{error, info, warn};
use once_cell::sync::OnceCell;
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

#[derive(Debug)]
pub struct PluginData {
    pub file_path: PathBuf,
    pub plugin: &'static BwsPlugin,
    pub raw_library: &'static Library,
}

pub static PLUGINS: OnceCell<Vec<PluginData>> = OnceCell::new();

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
        if let Err(e) = Version::parse(lib.plugin.version.into_str()) {
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

    // make sure the order of plugins is deterministic
    libs.sort_by(|a, b| a.file_path.cmp(&b.file_path));

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

    let deps = libs[lib].plugin.dependencies.into_slice();
    for dep in deps {
        let dep_name = dep.0.into_str();
        let version_req =
            VersionReq::parse(dep.1.into_str()).context("Couldn't parse version requirement")?;

        // first check if a plugin with the name exists
        match libs
            .iter()
            .find(|plugin| plugin.plugin.name.into_str() == dep_name)
        {
            Some(m) => {
                // Check if version matches
                let version = Version::parse(m.plugin.version.into_str())
                    .context("Couldn't parse version")?;

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

/// Calls init_fn of all the plugins
pub fn init_plugins() -> Result<()> {
    let plugins = PLUGINS.get().unwrap();

    let ordering = calc_ordering()?;

    // now that we know the order, we can init the plugins one by one
    for id in ordering {
        if (plugins[id].plugin.init_fn)(id, &crate::vtable::INIT_VTABLE)
            .unwrap()
            .into_result()
            .is_err()
        {
            bail!("Error initializing plugin {:?}", plugins[id].plugin.name);
        }
    }

    Ok(())
}

/// Calls start_fn of all the plugins
pub fn start_plugins() -> Result<()> {
    let plugins = PLUGINS.get().unwrap();

    // let plugins save vtable reference in memory
    for plugin in plugins {
        (plugin.plugin.vtable_fn)(&crate::vtable::VTABLE).unwrap();
    }

    let ordering = calc_ordering()?;

    // now that we know the order, we can start the plugins one by one
    for id in ordering {
        if (plugins[id].plugin.start_fn)()
            .unwrap()
            .into_result()
            .is_err()
        {
            bail!("Error starting plugin {:?}", plugins[id].plugin.name);
        }
    }

    Ok(())
}

fn calc_ordering() -> Result<Vec<usize>> {
    let plugins = PLUGINS.get().unwrap();

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

    let mut result = Vec::new();

    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        for (id, plugin) in plugins.iter().enumerate() {
            if plugin.plugin.name == *plugin_name {
                result.push(id);
                break;
            }
        }
    }

    Ok(result)
}
