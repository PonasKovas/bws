use rsa::RsaPrivateKey;

use crate::graceful_shutdown::ShutdownSystem;

/// Data storage required to operate a base server
#[derive(Debug)]
pub struct ServerBaseStore {
    pub(crate) shutdown: ShutdownSystem,
    pub(crate) rsa_keypair: RsaPrivateKey,
}

impl ServerBaseStore {
    pub fn new() -> Self {
        Self {
            shutdown: ShutdownSystem::new(),
            rsa_keypair: RsaPrivateKey::new(&mut rand::thread_rng(), 4096).unwrap(),
        }
    }
}
