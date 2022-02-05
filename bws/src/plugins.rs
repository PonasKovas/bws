use crate::LinearSearch;
use abi_stable::external_types::RRwLock;
use abi_stable::std_types::{RArc, RStr, RString};
use anyhow::{bail, Context, Result};
use bws_plugin_interface::global_state::plugin::Plugin;
use bws_plugin_interface::global_state::{GState, GlobalState};
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use log::{error, info, warn};
use repr_c_types::std::SArcOpaque;
use semver::{Version, VersionReq};
use std::fmt::Debug;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const PLUGIN_DIR: &str = "plugins/";

pub fn load_plugins() -> Result<Vec<Plugin>> {
    let mut libs = Vec::new();

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
            }
        }
    }

    // Check if dependencies are satisfied
    for lib in (0..libs.len()).rev() {
        if unsafe { check_dependencies(&libs, lib).context("Error checking dependencies")? } {
            info!(
                "loaded {} {} ({}).",
                libs[lib].name(),
                libs[lib].version(),
                libs[lib].path
            );
        } else {
            warn!(
                "Couldn't load {} {} ({}).",
                libs[lib].name(),
                libs[lib].version(),
                libs[lib].path
            );
            // remove the lib from the list
            libs.remove(lib);
        }
    }

    Ok(libs)
}

pub unsafe fn load_lib(path: impl AsRef<Path>) -> Result<Plugin> {
    let path = path.as_ref();

    let lib = unsafe { Library::new(path)? };
    let abi: Symbol<*const u32> = unsafe {
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

pub unsafe fn check_dependencies(libs: &[Plugin], lib: usize) -> Result<bool> {
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

pub fn start_plugins(global_state: &GState) -> Result<()> {
    // Use the graph theory to order the plugins so that they would load
    // only after all of their dependencies have loaded.

    let gstate_lock = global_state.read();
    let plugins = &gstate_lock.plugins;

    let mut graph = petgraph::graph::DiGraph::<RString, ()>::new();
    let mut indices: Vec<(RString, _)> = Vec::new();
    for plugin in plugins {
        let plugin = plugin.read();

        indices.push((plugin.name().into(), graph.add_node(plugin.name().into())));
    }

    // set the edges
    // (in other words, connect dependencies)
    for plugin in plugins {
        let plugin = plugin.read();

        let id = indices.search(&RString::from(plugin.name()));
        for dependency in plugin.dependencies().as_slice() {
            graph.update_edge(*indices.search(&RString::from(dependency.0)), *id, ());
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

    // drop the read-only global state lock
    drop(gstate_lock);

    // now that we know the order, we can start the plugins one by one
    for plugin_id in ordering {
        let plugin_name = indices.search_by_val(&plugin_id);

        let gstate_lock = global_state.read();

        let plugin = RArc::clone(
            gstate_lock
                .plugins
                .iter()
                .find(|p| &p.read().name() == plugin_name)
                .unwrap(),
        );

        drop(gstate_lock); // in case enable() needs global state

        if plugin.write().enable().is_err() {
            bail!("{} was already started", plugin_name);
        }

        info!("{} started.", plugin_name);
    }

    Ok(())
}
