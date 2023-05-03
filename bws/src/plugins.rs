use anyhow::{bail, Context, Result};
use bws_plugin_interface::ironties::types::{FfiSafeEquivalent, SStr};
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use once_cell::sync::OnceCell;
use semver::{Version, VersionReq};
use std::{
    collections::HashSet,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

const PLUGIN_DIR: &str = "plugins/";

#[derive(Debug)]
pub struct PluginData {
    pub file_path: PathBuf,
    pub plugin: &'static BwsPlugin,
    pub raw_library: Library,
}

pub static PLUGINS: OnceCell<Vec<PluginData>> = OnceCell::new();

pub fn load_plugins() -> Result<()> {
    let mut plugins = Vec::new();

    let mut success = true;

    recursive_read_dir(PLUGIN_DIR, &mut |path| match unsafe { load_lib(&path) } {
        Ok(l) => plugins.push(l),
        Err(e) => {
            eprintln!("Error loading {:?}: {e:?}", path.file_name().unwrap());
            success = false;
        }
    })?;

    // Make sure all plugins have unique names
    //////////////////////////////////////////

    let mut names = HashSet::new();
    let mut names_with_collisions = Vec::new();
    for plugin in &plugins {
        if !names.insert(plugin.plugin.name) {
            names_with_collisions.push(plugin.plugin.name);
            success = false;
        }
    }

    for name in &names_with_collisions {
        eprintln!(
            "Plugin name collision: {:?} is provided by the following libraries:",
            name
        );
        for plugin in &plugins {
            if plugin.plugin.name == *name {
                println!(" - {}", plugin.file_path.display(),);
            }
        }
    }

    // make sure the plugins provide APIs with valid versions
    /////////////////////////////////////////////////////////

    for plugin in &plugins {
        for api in plugin.plugin.provides {
            if let Err(e) = Version::parse(api.version.into_normal()) {
                eprintln!(
                    "{} version {:?} (from plugin {} ({})) could not be parsed: {e}",
                    api.name,
                    api.version,
                    plugin.plugin.name,
                    plugin.file_path.display(),
                );
                success = false;
            }
        }
    }

    // if any problems encountered up until this point, we can already return since checking the
    // dependencies is useless if the plugins cant even say their names or versions right
    if !success {
        bail!("Corrupted plugins found");
    }

    // Check if dependencies are satisfied
    for plugin in 0..plugins.len() {
        match check_dependencies(&plugins, plugin) {
            Ok(false) => {
                eprintln!(
                    "Dependencies of {} ({}) are not satisfied so it couldn't be loaded.",
                    plugins[plugin].plugin.name,
                    plugins[plugin].file_path.display()
                );
                success = false;
            }
            Err(e) => {
                eprintln!(
                    "Error checking dependencies of {} ({}): {e}",
                    plugins[plugin].plugin.name,
                    plugins[plugin].file_path.display()
                );
                success = false;
            }
            _ => {}
        }
    }

    if success {
        PLUGINS.set(plugins).expect("PLUGINS static already set!");
    } else {
        bail!("Some dependencies couldn't be satisfied.");
    }

    Ok(())
}

pub fn recursive_read_dir(path: impl AsRef<Path>, f: &mut dyn FnMut(PathBuf)) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();

        match path.file_name().unwrap().to_str() {
            None => {
                bail!(
                    "file name contains invalid unicode: {:?}",
                    path.file_name().unwrap()
                );
            }
            Some(file_name) => {
                // skip hidden files and directories
                if file_name.starts_with('.') {
                    continue;
                }
            }
        }

        if path.is_dir() {
            recursive_read_dir(&path, f)?;
        } else {
            f(path);
        }
    }

    Ok(())
}

pub unsafe fn load_lib(path: impl AsRef<Path>) -> Result<PluginData> {
    let path = path.as_ref();

    let lib = unsafe { Library::new(path)? };
    let abi: Symbol<*const SStr<'static>> = unsafe {
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
        lib.get::<*const BwsPlugin>(b"BWS_PLUGIN")
            .context("BWS_PLUGIN not found")?
    };

    Ok(PluginData {
        file_path: path.to_path_buf(),
        plugin: unsafe { root.as_ref().unwrap() },
        raw_library: lib,
    })
}

/// Returns `true` if dependencies satisfied
pub fn check_dependencies(plugins: &[PluginData], id: usize) -> Result<bool> {
    let mut res = true;

    let deps = plugins[id].plugin.depends_on.into_normal();
    for dep in deps {
        let dep_name = dep.0;
        let version_req =
            VersionReq::parse(dep.1.into_normal()).context("Couldn't parse version requirement")?;

        // first check if an API with the name exists
        /////////////////////////////////////////////

        let api = match plugins
            .iter()
            .flat_map(|plugin| plugin.plugin.provides)
            .find(|api| api.name == dep_name)
        {
            Some(api) => api,
            None => {
                eprintln!(
                    "{}: needs {dep_name} {version_req} which wasn't found.",
                    plugins[id].plugin.name,
                );
                res = false;
                continue;
            }
        };

        // Check if the versions match
        //////////////////////////////

        let version =
            Version::parse(api.version.into_normal()).context("Couldn't parse version")?;
        if !version_req.matches(&version) {
            eprintln!(
                "{}: needs {dep_name} {version_req} which wasn't found. {dep_name} {version} found, but versions incompatible.",
                plugins[id].plugin.name
            );
            res = false;
        }
    }

    Ok(res)
}

/// Starts all plugins in an order that they only start
/// after all of their dependencies have started
pub fn start_plugins() -> Result<()> {
    let plugins = PLUGINS.get().unwrap();

    let ordering = calc_ordering(plugins)?;

    for id in ordering {
        (plugins[id].plugin.start)(id, &crate::vtable::VTABLE).unwrap();
    }

    Ok(())
}

fn calc_ordering(plugins: &Vec<PluginData>) -> Result<Vec<usize>> {
    // Use the graph theory to order the plugins so that they would load
    // only after all of their dependencies have loaded.

    let mut graph = petgraph::graph::DiGraph::<SStr<'static>, ()>::new();
    let indices: Vec<_> = plugins
        .iter()
        .map(|p| graph.add_node(p.plugin.name))
        .collect();

    // set the edges
    // (in other words, connect dependencies)
    for (plugin_id, id) in indices.iter().enumerate() {
        for dependency in plugins[plugin_id].plugin.depends_on {
            // Now iterate through all plugins again and check which one provides the dependency
            for (dep_id, dep_plugin) in plugins.iter().enumerate() {
                if dep_plugin
                    .plugin
                    .provides
                    .iter()
                    .find(|api| api.name == dependency.0)
                    .is_some()
                {
                    graph.update_edge(indices[dep_id], *id, ());
                    break;
                }
            }
        }
    }

    // perform a topological sort of the nodes 😎
    let ordering = match petgraph::algo::toposort(&graph, None) {
        Ok(o) => o,
        Err(cycle) => {
            bail!(
                "Dependency cycle detected: {}",
                plugins[indices
                    .iter()
                    .position(|id| *id == cycle.node_id())
                    .unwrap()]
                .plugin
                .name
            );
        }
    };

    let mut result = Vec::new();

    for node_id in ordering {
        result.push(indices.iter().position(|id| *id == node_id).unwrap())
    }

    Ok(result)
}
