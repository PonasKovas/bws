use bws_plugin_interface::{plugin_api::PluginApiPtr, safe_types::*};

pub extern "C" fn get_plugin_vtable(
    _plugin_id: usize,
    plugin: SStr,
) -> MaybePanicked<PluginApiPtr> {
    MaybePanicked::new(move || {
        crate::plugins::PLUGINS
            .get()
            .unwrap()
            .iter()
            .find(|x| x.plugin.name == plugin)
            .unwrap()
            .plugin
            .api
            .into_option()
            .expect("plugin does not expose an API")
    })
}
