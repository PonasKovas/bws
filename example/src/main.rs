use bws::serverbase;

fn main() {
    struct MyServer {
        serverbase_store: serverbase::ServerBaseStore,
    }
    impl serverbase::ServerBase for MyServer {
        fn store(&self) -> &serverbase::ServerBaseStore {
            &self.serverbase_store
        }
        fn store_mut(&mut self) -> &mut serverbase::ServerBaseStore {
            &mut self.serverbase_store
        }
    }

    let my_server = MyServer {
        serverbase_store: serverbase::ServerBaseStore::new(),
    };

    if let Err(e) = bws::application::run_app(my_server, &Default::default()) {
        eprintln!("Error running server: {:?}", e)
    }
}
