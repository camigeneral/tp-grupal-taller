extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{BoxExt, EditableExt, EntryExt, GridExt, OrientableExt, WidgetExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Clone)]
pub struct Cell {
    raw_content: String,
    calculated_value: f64,
    display_text: String,
    is_formula: bool,
}

impl Cell {
    fn new() -> Self {
        Cell {
            raw_content: String::new(),
            calculated_value: 0.0,
            display_text: String::new(),
            is_formula: false,
        }
    }
}

#[derive(Debug)]
pub struct SpreadsheetModel {
    cells: Vec<Vec<Cell>>,
    entries: Vec<Vec<gtk::Entry>>,
}

#[derive(Debug)]
pub enum SpreadsheetMsg {
    CellChanged(usize, usize, String),
    RecalculateAll,
    UpdateSheet(String, Vec<Vec<String>>)
}

#[derive(Debug)]
pub enum SpreadsheetOutput {
    ContentChanged(String, String),
    GoBack,
}

impl SpreadsheetModel {
    fn parse_cell_reference(&self, cell_ref: &str) -> Option<(usize, usize)> {
        if cell_ref.len() < 2 {
            return None;
        }

        let mut chars = cell_ref.chars();
        let col_char = chars.next()?;
        let row_str: String = chars.collect();

        if !col_char.is_ascii_alphabetic() {
            return None;
        }

        let col = (col_char.to_ascii_uppercase() as u8 - b'A') as usize;
        let row = row_str.parse::<usize>().ok()?.saturating_sub(1);

        if row < 10 && col < 10 {
            Some((row, col))
        } else {
            None
        }
    }

    fn evaluate_expression(&self, expr: &str) -> Result<f64, String> {
        let expr = expr.trim();

        if let Ok(num) = expr.parse::<f64>() {
            return Ok(num);
        }

        if let Some((row, col)) = self.parse_cell_reference(expr) {
            return Ok(self.cells[row][col].calculated_value);
        }

        self.evaluate_arithmetic(expr)
    }

    fn evaluate_arithmetic(&self, expr: &str) -> Result<f64, String> {
        let mut processed_expr = expr.to_string();

        for i in 0..10 {
            for j in 0..10 {
                let cell_name = format!("{}{}", (b'A' + j as u8) as char, i + 1);
                if processed_expr.contains(&cell_name) {
                    let value = self.cells[i][j].calculated_value;
                    processed_expr = processed_expr.replace(&cell_name, &value.to_string());
                }
            }
        }
        self.simple_calculator(&processed_expr)
    }
    // Evaluación recursiva con procedencia de operadores
    fn simple_calculator(&self, expr: &str) -> Result<f64, String> {
        let expr = expr.replace(" ", "");
        if let Some(pos) = expr.rfind('+') {
            let left = self.simple_calculator(&expr[..pos])?;
            let right = self.simple_calculator(&expr[pos + 1..])?;
            return Ok(left + right);
        }

        if let Some(pos) = expr.rfind('-') {
            //TODO: arreglar manejo de negativos
            if pos > 0 {
                let left = self.simple_calculator(&expr[..pos])?;
                let right = self.simple_calculator(&expr[pos + 1..])?;
                return Ok(left - right);
            }
        }
        if let Some(pos) = expr.rfind('*') {
            let left = self.simple_calculator(&expr[..pos])?;
            let right = self.simple_calculator(&expr[pos + 1..])?;
            return Ok(left * right);
        }

        if let Some(pos) = expr.rfind('/') {
            let left = self.simple_calculator(&expr[..pos])?;
            let right = self.simple_calculator(&expr[pos + 1..])?;
            if right == 0.0 {
                return Err("División por cero".to_string());
            }
            return Ok(left / right);
        }

        expr.parse::<f64>()
            .map_err(|_| format!("Expresión inválida: {}", expr))
    }

    fn update_cell(&mut self, row: usize, col: usize, content: String) {
        self.cells[row][col].raw_content = content.clone();

        if content.starts_with('=') && content.len() > 1 {
            self.cells[row][col].is_formula = true;
            let formula = &content[1..];

            match self.evaluate_expression(formula) {
                Ok(value) => {
                    self.cells[row][col].calculated_value = value;
                    self.cells[row][col].display_text = value.to_string();
                }
                Err(error) => {
                    self.cells[row][col].calculated_value = 0.0;
                    self.cells[row][col].display_text = format!("#ERROR: {}", error);
                }
            }
        } else if content.starts_with('=') && content.len() == 1 {
            self.cells[row][col].is_formula = false;
            self.cells[row][col].calculated_value = 0.0;
            self.cells[row][col].display_text = content;
        } else {
            self.cells[row][col].is_formula = false;

            match content.parse::<f64>() {
                Ok(value) => {
                    self.cells[row][col].calculated_value = value;
                    self.cells[row][col].display_text = content;
                }
                Err(_) => {
                    self.cells[row][col].calculated_value = 0.0;
                    self.cells[row][col].display_text = content;
                }
            }
        }
    }

    fn recalculate_all(&mut self) {
        for i in 0..10 {
            for j in 0..10 {
                if self.cells[i][j].is_formula {
                    let formula = self.cells[i][j].raw_content[1..].to_string();
                    match self.evaluate_expression(&formula) {
                        Ok(value) => {
                            self.cells[i][j].calculated_value = value;
                            self.cells[i][j].display_text = value.to_string();
                        }
                        Err(error) => {
                            self.cells[i][j].calculated_value = 0.0;
                            self.cells[i][j].display_text = format!("#ERROR: {}", error);
                        }
                    }
                }
            }
        }
    }

    fn update_display(&self) {
        for i in 0..10 {
            for j in 0..10 {
                self.entries[i][j].set_text(&self.cells[i][j].display_text);
            }
        }
    }
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

            gtk::Label {
                set_text: "Instrucciones: Ingresa números o fórmulas (ej: =A1+B1, =A1*2, =A1+B1-C1) y presiona ENTER",
                set_margin_top: 5,
                set_margin_bottom: 5,
                set_margin_start: 5,
                set_margin_end: 5,
            },

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
        let mut model = SpreadsheetModel {
            cells: vec![vec![Cell::new(); 10]; 10],
            entries: vec![vec![]; 10],
        };

        let widgets = view_output!();

        for i in 0..10 {
            model.entries[i] = Vec::new();
            for j in 0..10 {
                let entry = gtk::Entry::new();
                entry.set_width_chars(10);
                entry.set_max_width_chars(15);
                entry.add_css_class("spreadsheet-cell");

                let cell_name = format!("{}{}", (b'A' + j as u8) as char, i + 1);
                entry.set_placeholder_text(Some(&cell_name));

                let row = i;
                let col = j;
                let sender_clone = sender.clone();

                entry.connect_activate(move |e| {
                    sender_clone.input(SpreadsheetMsg::CellChanged(row, col, e.text().to_string()));
                });

                widgets.grid.attach(&entry, j as i32, i as i32, 1, 1);
                model.entries[i].push(entry);
            }
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SpreadsheetMsg::CellChanged(row, col, content) => {
                self.update_cell(row, col, content);
                self.recalculate_all();
                self.update_display();
                let cell_name = format!("{}{}", (b'A' + col as u8) as char, row + 1);
                sender
                    .output(SpreadsheetOutput::ContentChanged(
                        cell_name,
                        self.cells[row][col].display_text.clone(),
                    ))
                    .unwrap();
            }
            SpreadsheetMsg::RecalculateAll => {
                self.recalculate_all();
                self.update_display();
            }
            SpreadsheetMsg::UpdateSheet(_file_name, filas) => {
                // Actualiza las celdas con los datos recibidos
                for i in 0..10 {
                    for j in 0..10 {
                        let value = filas.get(i).and_then(|row| row.get(j)).cloned().unwrap_or_default();
                        self.cells[i][j] = Cell::new();
                        self.update_cell(i, j, value);
                    }
                }
                self.update_display();
            }
        }
    }
}
