use crate::graceful_shutdown::ShutdownSystem;

/// Data storage required to operate a base server
#[derive(Debug)]
pub struct ServerBaseStore {
    pub shutdown: ShutdownSystem,
}

impl ServerBaseStore {
    pub fn new() -> Self {
        Self {
            shutdown: ShutdownSystem::new(),
        }
    }
}
