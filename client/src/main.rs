extern crate relm4;
extern crate rusty_docs;
use relm4::RelmApp;
mod app;
use app::AppModel;
extern crate rand;
use rand::Rng;
use std::env::args;
mod components;
mod client;
mod types;

fn main() {
    let cli_args: Vec<String> = args().collect();

    let port = match cli_args[1].parse::<u16>() {
        Ok(n) => n,
        Err(_e) => return,
    };

    let id = format!("rusty.docs{}", rand::thread_rng().gen_range(0..100));
    let app = RelmApp::new(&id);
    app.with_args(vec![]).run::<AppModel>(port);
}
