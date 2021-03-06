use crate::LinearSearch;
use abi_stable::external_types::RRwLock;
use abi_stable::std_types::{RArc, RStr, RString};
use anyhow::{bail, Context, Result};
use bws_plugin_interface::global_state::plugin::Plugin;
use bws_plugin_interface::global_state::{GState, GlobalState};
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use log::{error, info, warn};
use safe_types::std::sync::SArcOpaque;
use semver::{Version, VersionReq};
use std::fmt::Debug;
use std::thread;
use std::time::Duration;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const PLUGIN_DIR: &str = "plugins/";

pub fn load_plugins() -> Result<Vec<Plugin>> {
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

    // Check if dependencies are satisfied
    for lib in (0..libs.len()).rev() {
        if check_dependencies(&libs, lib).context("Error checking dependencies")? {
            info!(
                "loaded {} {} ({}).",
                libs[lib].name(),
                libs[lib].version(),
                libs[lib].path()
            );
        } else {
            error!(
                "Couldn't load {} {} ({}).",
                libs[lib].name(),
                libs[lib].version(),
                libs[lib].path()
            );
            success = false;
        }
    }

    if success {
        Ok(libs)
    } else {
        bail!("Some plugins couldn't be loaded.");
    }
}

pub unsafe fn load_lib(path: impl AsRef<Path>) -> Result<Plugin> {
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

    Ok(Plugin::new(
        RString::from(
            path.to_str()
                .context("Library path must be valid unicode")?,
        ),
        SArcOpaque::new(Arc::new(lib)),
        unsafe { root.as_ref().unwrap() },
    ))
}

pub fn check_dependencies(libs: &[Plugin], lib: usize) -> Result<bool> {
    let mut res = true;

    let deps = libs[lib].dependencies().as_slice();
    for dep in deps {
        let dep_name = dep.0.as_str();
        let version_req =
            VersionReq::parse(dep.1.as_str()).context("Couldn't parse version requirement")?;

        // first check if a plugin with the name exists
        match libs
            .iter()
            .find(|plugin| plugin.name().as_str() == dep_name)
        {
            Some(m) => {
                // Check if version matches
                let version =
                    Version::parse(m.version().as_str()).context("Couldn't parse version")?;

                if !version_req.matches(&version) {
                    error!(
                        "{}: needs {dep_name} {version_req} which wasn't found. {dep_name} {version} found, but versions incompatible.",
                        libs[lib].name()
                    );
                    res = false;
                }
            }
            None => {
                error!(
                    "{}: needs {dep_name} {version_req} which wasn't found.",
                    libs[lib].name(),
                );
                res = false;
            }
        }
    }

    Ok(res)
}

pub fn start_plugins(gstate: &GState) -> Result<()> {
    // Use the graph theory to order the plugins so that they would load
    // only after all of their dependencies have loaded.

    let plugins_lock = gstate.plugins.read();
    let plugins = &plugins_lock.0;

    let mut graph = petgraph::graph::DiGraph::<RString, ()>::new();
    let mut indices: Vec<(RString, _)> = Vec::new();
    for plugin in plugins {
        indices.push((plugin.0.clone(), graph.add_node(plugin.0.clone())));
    }

    // set the edges
    // (in other words, connect dependencies)
    for plugin in plugins {
        let id = indices.search(&plugin.0.clone());
        for dependency in plugin.1.dependencies().as_slice() {
            graph.update_edge(*indices.search(&RString::from(dependency.0)), *id, ());
        }
    }

    // perform a topological sort of the nodes ????
    let ordering = match petgraph::algo::toposort(&graph, None) {
        Ok(o) => o,
        Err(cycle) => {
            bail!(
                "Dependency cycle detected: {}",
                indices.search_by_val(&cycle.node_id())
            );
        }
    };

    // drop the read-only plugins lock
    // so we could lock and unlock every iteration below:
    drop(plugins_lock);

    // now that we know the order, we can start the plugins one by one
    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        let plugins_lock = gstate.plugins.read();

        let plugin = RArc::clone(plugins_lock.get(plugin_name.as_rstr()).unwrap());

        drop(plugins_lock); // in case enable() needs plugins

        if plugin.enable(&gstate).is_err() {
            bail!("{} was already started", plugin_name);
        }

        info!("{} started.", plugin_name);
    }

    Ok(())
}
