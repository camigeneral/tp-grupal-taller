extern crate relm4;
extern crate rusty_docs;
use relm4::RelmApp;
use rusty_docs::app::AppModel;
use std::thread;

mod node;
use node::start_server;

fn main() {

    let port = 4000;

    thread::spawn(move || {
        if let Err(e) = start_server(port) {
            eprintln!("Error al iniciar el servidor: {:?}", e);
        }
    });

    let app = RelmApp::new("rusty.docs");
    app.run::<AppModel>(port);
    
}
