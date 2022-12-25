use crate::plugins::PluginData;
use bws_plugin_interface::{
    safe_types::*,
    vtable::{LogLevel, VTable},
    PluginApi,
};
use once_cell::sync::{Lazy, OnceCell};
use std::sync::Mutex;

pub static PLUGINS: OnceCell<Vec<PluginData>> = OnceCell::new();

pub extern "C" fn get_plugin_vtable(plugin: SStr) -> PluginApi {
    PLUGINS
        .get()
        .unwrap()
        .iter()
        .find(|x| x.plugin.name == plugin)
        .unwrap()
        .plugin
        .api
}
