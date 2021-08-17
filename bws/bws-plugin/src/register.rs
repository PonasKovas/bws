use crate::*;

pub type _f_PluginEntry =
    unsafe extern "C" fn(BwsStr, PluginGate, BwsGlobalState) -> FfiFuture<Unit>;
pub type _f_SubPluginEntry = unsafe extern "C" fn(BwsStr, SubPluginGate) -> FfiFuture<Unit>;

#[derive(Clone)]
pub struct Plugin {
    pub name: String,
    pub version: (u64, u64, u64),
    pub dependencies: Vec<(String, String)>,
    pub subscribed_events: Vec<String>,
    pub subplugins: Vec<SubPlugin>,
    pub entry: _f_PluginEntry,
}

#[derive(Clone)]
pub struct SubPlugin {
    pub name: String,
    pub subscribed_events: Vec<String>,
    pub entry: _f_SubPluginEntry,
}

impl Plugin {
    pub fn new(name: String, version: (u64, u64, u64), entry: _f_PluginEntry) -> Self {
        Self {
            name,
            version,
            dependencies: Vec::new(),
            subscribed_events: Vec::new(),
            subplugins: Vec::new(),
            entry,
        }
    }
    pub fn add_event(mut self, event: impl AsRef<str>) -> Self {
        self.subscribed_events.push(event.as_ref().to_owned());

        self
    }
    pub fn add_dep(
        mut self,
        dependency_name: impl AsRef<str>,
        dependency_version_req: impl AsRef<str>,
    ) -> Self {
        self.dependencies.push((
            dependency_name.as_ref().to_owned(),
            dependency_version_req.as_ref().to_owned(),
        ));

        self
    }
    pub fn add_subplugin(mut self, subplugin: SubPlugin) -> Self {
        self.subplugins.push(subplugin);

        self
    }
    pub fn register(self) {
        let plugin_id = unsafe {
            (crate::vtable::VTABLE.register_plugin)(
                BwsStr::from_str(&self.name),
                Tuple3(self.version.0, self.version.1, self.version.2),
                BwsSlice::from_slice(
                    &self
                        .dependencies
                        .iter()
                        .map(|(name, version_req)| {
                            Tuple2(BwsStr::from_str(&name), BwsStr::from_str(&version_req))
                        })
                        .collect::<Vec<_>>()[..],
                ),
                BwsSlice::from_slice(
                    &self
                        .subscribed_events
                        .iter()
                        .map(|e| BwsStr::from_str(&e))
                        .collect::<Vec<_>>()[..],
                ),
                self.entry,
            )
        };

        for subplugin in self.subplugins {
            unsafe {
                (crate::vtable::VTABLE.register_subplugin)(
                    plugin_id,
                    BwsStr::from_str(&subplugin.name),
                    BwsSlice::from_slice(
                        &self
                            .subscribed_events
                            .iter()
                            .map(|e| BwsStr::from_str(&e))
                            .collect::<Vec<_>>()[..],
                    ),
                    subplugin.entry,
                );
            }
        }
    }
}

impl SubPlugin {
    pub fn new(name: String, entry: _f_SubPluginEntry) -> Self {
        Self {
            name,
            subscribed_events: Vec::new(),
            entry,
        }
    }
}
