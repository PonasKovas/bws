use anyhow::{bail, Context, Result};
use libloading::{Library, Symbol};
use log::info;

pub fn load_plugins() -> Result<()> {
    let lib = unsafe {
        Library::new("/home/mykolas/Projects/rust/bws_plugin_template/target/debug/libbws_plugin_template.so")?
    };
    let abi: Symbol<*const u32> = unsafe { lib.get(b"BWS_ABI")? };

    info!("ABI: {}", unsafe { **abi });

    let root: Symbol<*const bws_plugin_interface::BwsPlugin> =
        unsafe { lib.get(b"BWS_PLUGIN_ROOT")? };

    info!("name {}, version {}", unsafe { (**root).name }, unsafe {
        (**root).version
    });

    Ok(())
}
