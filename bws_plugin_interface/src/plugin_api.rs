use bws_plugin_api::PluginApi;

/// Allows plugins to define their own API for other plugins to use
#[derive_ReprC]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PluginApiPtr {
    vtable: NonNullRef<c_void>,
    // for sanity checks
    pkg_name: str_ref<'static>,
    pkg_version: str_ref<'static>,
}

impl PluginApiPtr {
    /// Converts a `PluginApi` reference to to a `PluginApiPtr`
    pub const fn new<T: PluginApi>(plugin_api: &'static T) -> Self {
        Self {
            vtable: plugin_api.into::<NonNullRef<_>>().cast(),
            pkg_name: <T as PluginApi>::PKG_NAME,
            pkg_version: <T as PluginApi>::PKG_VERSION,
        }
    }
    pub unsafe fn get<T: PluginApi>(&self) -> &'static T {
        // check if T is compatible with the original
        debug_assert_eq!(
            <T as PluginApi>::PKG_NAME,
            self.pkg_name.as_str(),
            "plugin api pkg names dont match"
        );

        fn split(version: &str) -> Option<(u64, u64, u64)> {
            let mut s = version.split('.');

            let result = (
                s.next()?.parse().ok()?,
                s.next()?.parse().ok()?,
                s.next()?.parse().ok()?,
            );

            // make sure theres no fourth segment
            if s.next().is_some() {
                return None;
            }

            Some(result)
        }

        let (my_major, my_minor, _my_patch) =
            split(<T as PluginApi>::PKG_VERSION).expect("incorrect PKG_VERSION format");
        let (og_major, og_minor, _og_patch) =
            split(self.pkg_version.as_str()).expect("incorrect PKG_VERSION format");

        if og_major == 0 {
            debug_assert_eq!(
                <T as PluginApi>::PKG_VERSION,
                self.pkg_version.as_str(),
                "plugin api pkg versions are not compatible"
            );
        } else {
            debug_assert_eq!(
                (my_major, my_minor) == (og_major, og_minor),
                "plugin api pkg versions are not compatible"
            );
        }

        unsafe { self.vtable.cast::<T>().as_ref() }
    }
}

unsafe impl Sync for PluginApiPtr {}
unsafe impl Send for PluginApiPtr {}
