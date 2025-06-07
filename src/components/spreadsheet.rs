extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{BoxExt, OrientableExt, WidgetExt, GridExt, EditableExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug)]
pub struct SpreadsheetModel {
    cells: Vec<Vec<String>>,
}

#[derive(Debug)]
pub enum SpreadsheetMsg {
    CellChanged(usize, usize, String),
}

#[derive(Debug)]
pub enum SpreadsheetOutput {
    ContentChanged(String),
    GoBack,
}

#[relm4::component(pub)]
impl SimpleComponent for SpreadsheetModel {
    type Init = ();
    type Input = SpreadsheetMsg;
    type Output = SpreadsheetOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_hexpand: true,
            set_vexpand: true,

            gtk::ScrolledWindow {
                set_hexpand: true,
                set_vexpand: true,
                
                #[name="grid"]
                gtk::Grid {
                    set_row_spacing: 1,
                    set_column_spacing: 1,
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = SpreadsheetModel {
            cells: vec![vec![String::new(); 10]; 10],
        };

        let widgets = view_output!();

        for i in 0..10 {
            for j in 0..10 {
                let entry = gtk::Entry::new();
                entry.set_width_chars(8);
                entry.set_max_width_chars(8);
                entry.add_css_class("spreadsheet-cell");
                
                let row = i;
                let col = j;
                let sender_clone = sender.clone();
                
                entry.connect_changed(move |e| {
                    sender_clone.input(SpreadsheetMsg::CellChanged(
                        row,
                        col,
                        e.text().to_string(),
                    ));
                });
                
                widgets.grid.attach(&entry, j as i32, i as i32, 1, 1);
            }
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SpreadsheetMsg::CellChanged(row, col, content) => {
                self.cells[row][col] = content;

                println!("Celda [{},{}] cambió a: {}", row, col, self.cells[row][col]);
                sender.output(SpreadsheetOutput::ContentChanged(
                    format!("Celda [{},{}] cambió a: {}", row, col, self.cells[row][col])
                )).unwrap();
            }
        }
    }
}
