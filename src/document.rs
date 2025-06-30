#[derive(Debug, Clone, PartialEq)]
pub enum Document {
    Text(Vec<String>),
    Spreadsheet(Vec<String>),
}

impl Default for Document {
    fn default() -> Self {
        Document::Text(Vec::new())
    }
}