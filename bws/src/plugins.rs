use abi_stable::std_types::RString;
use anyhow::{bail, Context, Result};
use bws_plugin_interface::BwsPlugin;
use libloading::{Library, Symbol};
use log::{error, info, warn};
use semver::{Version, VersionReq};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const PLUGIN_DIR: &str = "plugins/";

/// Drops the Arc when dropped, without exposing it's type
/// And being FFI-safe
#[repr(C)]
pub struct FfiOpaqueArc {
    raw: *const (),
    drop: unsafe fn(*const ()),
}

impl FfiOpaqueArc {
    pub fn new<T>(arc: Arc<T>) -> Self {
        let raw = Arc::into_raw(arc) as *const ();

        unsafe fn drop<T>(raw: *const ()) {
            unsafe {
                Arc::from_raw(raw as *const T);
            }
        }
        Self {
            raw,
            drop: drop::<T>,
        }
    }
}

impl Drop for FfiOpaqueArc {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(self.raw);
        }
    }
}

#[repr(C)]
pub struct Plugin {
    pub path: RString,
    // Basically only here to make sure that Library stays in memory
    // as long as this struct exists
    pub library: FfiOpaqueArc,
    // Valid as long as the library above is valid
    // exposed through a method to make sure the reference doesn't outlive the struct
    root: &'static BwsPlugin,
}

impl Plugin {
    // This method is zero-cost, the only reason it's not a field is because it
    // needs to expose root with `'a` lifetime
    pub fn root<'a>(&'a self) -> &'a BwsPlugin {
        self.root
    }
}

pub fn load_plugins() -> Result<()> {
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
    for lib in 0..libs.len() {
        if unsafe { check_dependencies(&libs, lib).context("Error checking dependencies")? } {
            info!(
                "loaded {} {} ({}).",
                libs[lib].root().name,
                libs[lib].root().version,
                libs[lib].path
            );
        } else {
            warn!(
                "Couldn't load {} {} ({}).",
                libs[lib].root().name,
                libs[lib].root().version,
                libs[lib].path
            );
        }
    }

    Ok(())
}

unsafe fn load_lib(path: impl AsRef<Path>) -> Result<Plugin> {
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

    Ok(Plugin {
        path: RString::from(path.to_str().context("Invalid library path")?),
        library: FfiOpaqueArc::new(Arc::new(lib)),
        root: unsafe { root.as_ref().unwrap() },
    })
}

unsafe fn check_dependencies(libs: &[Plugin], lib: usize) -> Result<bool> {
    let root = libs[lib].root();

    let mut res = true;

    let deps = root.dependencies.as_slice();
    for dep in deps {
        let dep_name = dep.0.as_str();
        let version_req =
            VersionReq::parse(dep.1.as_str()).context("Couldn't parse version requirement")?;

        // first check if a plugin with the name exists
        match libs
            .iter()
            .find(|plugin| plugin.root().name.as_str() == dep_name)
        {
            Some(m) => {
                // Check if version matches
                let version =
                    Version::parse(m.root().version.as_str()).context("Couldn't parse version")?;

                if !version_req.matches(&version) {
                    error!(
                        "{}: needs {dep_name} {version_req} which wasn't found. {dep_name} {version} found, but versions incompatible.",
                        root.name
                    );
                    res = false;
                }
            }
            None => {
                error!(
                    "{}: needs {dep_name} {version_req} which wasn't found.",
                    root.name,
                );
                res = false;
            }
        }
    }

    Ok(res)
}
