extern crate relm4;
extern crate rusty_docs;
use relm4::RelmApp;
use rusty_docs::app::AppModel;

fn main() {
    let app = RelmApp::new("rusty.docs");
    app.run::<AppModel>(());
}
