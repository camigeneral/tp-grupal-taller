extern crate relm4;
extern crate rusty_docs;
use relm4::RelmApp;
use rusty_docs::app::AppModel;
extern crate rand;
use rand::Rng;

fn main() {
    let id = format!("rusty.docs{}", rand::thread_rng().gen_range(0..100));
    let app = RelmApp::new(&id);
    let redis_port = 4000;
    app.run::<AppModel>(redis_port);
}
