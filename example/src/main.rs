use bws::serverbase::{ServerBase, ServerBaseStore};

struct MyServer {
    serverbase_store: ServerBaseStore,
}

impl ServerBase for MyServer {
    fn store(&self) -> &ServerBaseStore {
        &self.serverbase_store
    }
    fn store_mut(&mut self) -> &mut ServerBaseStore {
        &mut self.serverbase_store
    }
}

fn main() {
    let my_server = MyServer {
        serverbase_store: ServerBaseStore::new(),
    };

    if let Err(e) = bws::application::run_app(my_server, &Default::default()) {
        eprintln!("Error running server: {:?}", e)
    }
}
