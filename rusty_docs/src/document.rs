/// Enum que representa los distintos tipos de documentos soportados por el sistema.
///
/// - `Text`: Documento de texto plano, representado como un vector de líneas.
/// - `Spreadsheet`: Documento tipo planilla de cálculo, representado como un vector de filas.
#[derive(Debug, Clone, PartialEq)]
pub enum Document {
    Text(Vec<String>),
    Spreadsheet(Vec<String>),
}

impl Default for Document {
    /// Retorna un documento vacío de tipo texto por defecto.
    fn default() -> Self {
        Document::Text(Vec::new())
    }
}
