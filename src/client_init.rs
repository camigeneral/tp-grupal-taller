extern crate relm4;
extern crate rusty_docs;
use relm4::RelmApp;
use rusty_docs::app::AppModel;
extern crate rand;
use rand::Rng;

fn main() {
    let microservice_port = 5000;
    let id = format!("rusty.docs{}", rand::thread_rng().gen_range(0..100));
    let app = RelmApp::new(&id);
    app.run::<AppModel>(microservice_port);
}
