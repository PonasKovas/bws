use super::GState;
use crate::BwsPlugin;
use abi_stable::{
    external_types::{parking_lot::rw_lock::RReadGuard, RRwLock},
    std_types::{RArc, RSlice, RStr, RString, RVec, Tuple2},
};
use safe_types::std::sync::SArcOpaque;
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

#[repr(transparent)]
/// (plugin_name, plugin)
pub struct PluginList(pub RVec<Tuple2<RString, RArc<Plugin>>>);

impl Deref for PluginList {
    type Target = RVec<Tuple2<RString, RArc<Plugin>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PluginList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PluginList {
    pub fn get(&self, name: RStr) -> Option<&RArc<Plugin>> {
        self.iter().find(|p| p.0.as_rstr() == name).map(|p| &p.1)
    }
}

#[repr(C)]
pub struct Plugin {
    /// path to the file which provides this plugin
    path: RString,
    /// Basically only here to make sure that Library stays in memory
    /// as long as this struct exists
    library: SArcOpaque,
    // Valid as long as the library above is valid
    // exposed only through a method to make sure the reference doesn't outlive the struct
    root: &'static BwsPlugin,
    // Whether the plugin is enabled
    enabled: RRwLock<bool>,
}

impl Plugin {
    /// Constructs a new Plugin [For internal use only]
    pub fn new(path: RString, library: SArcOpaque, root: &'static BwsPlugin) -> Self {
        Self {
            path,
            library,
            root,
            enabled: RRwLock::new(false),
        }
    }
    /// Returns the path of the file that provides this plugin
    pub fn path(&self) -> &RString {
        &self.path
    }
    /// Returns None if the plugin is not enabled
    ///
    /// Avoid holding this lock for a long time
    pub fn root<'a>(&'a self) -> Option<RootGuard<'a>> {
        let enabled_lock = self.enabled.read();
        if !*enabled_lock {
            return None;
        }

        Some(RootGuard {
            root: self.root,
            _enabled_lock: enabled_lock,
        })
    }
    pub fn name(&self) -> RStr {
        self.root.name
    }
    pub fn version(&self) -> RStr {
        self.root.version
    }
    pub fn dependencies(&self) -> RSlice<Tuple2<RStr, RStr>> {
        self.root.dependencies
    }
    /// Whether the plugin is enabled
    pub fn enabled(&self) -> bool {
        *self.enabled.read()
    }
    /// Enables the plugin. Returns Err if was already enabled
    /// otherwise Ok
    pub fn enable(&self, gstate: &GState) -> Result<(), ()> {
        if *self.enabled.read() {
            return Err(());
        }

        *self.enabled.write() = true;
        (self.root.enable)(gstate);

        Ok(())
    }
    /// Disables the plugin. Returns Err if was already disabled
    /// otherwise Ok
    pub fn disable(&self, gstate: &GState) -> Result<(), ()> {
        if !*self.enabled.read() {
            return Err(());
        }

        *self.enabled.write() = false;
        (self.root.disable)(gstate);

        Ok(())
    }
}

/// Doesn't let anyone disable the plugin
/// until this guard is dropped
pub struct RootGuard<'a> {
    pub root: &'a BwsPlugin,
    _enabled_lock: RReadGuard<'a, bool>,
}

impl<'a> Deref for RootGuard<'a> {
    type Target = &'a BwsPlugin;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}
impl<'a> DerefMut for RootGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}
