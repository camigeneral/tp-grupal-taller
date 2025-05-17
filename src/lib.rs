// pub mod redis_commands;
// pub mod parse;
// pub mod client_info;
// pub mod redis_response;
// pub mod list_commands;
// pub mod set_commands;
// pub mod string_commands;
// pub mod pub_sub_commands;
/// Módulo principal de la aplicación.
///
/// Este módulo define la estructura de la aplicación, incluyendo los componentes principales como
/// la barra de navegación y el área de gestión de archivos. También maneja la comunicación entre
/// los componentes a través de los controladores y los mensajes.
///
/// # Componentes principales:
/// - `AppModel`: Modelo principal que contiene los controladores para los componentes.
/// - `AppMsg`: Enum de mensajes que controla las interacciones de la aplicación, como la conexión.
pub mod app;

/// Módulo de componentes que contiene las definiciones de los diferentes elementos UI.
///
/// Este módulo incluye todos los submódulos relacionados con la interfaz de usuario, como la gestión
/// de archivos, el encabezado, y la edición de archivos. Cada submódulo define un componente y su lógica.
/// Algunos de los componentes definidos en este módulo son:
/// - `file_workspace`: Gestiona el área de trabajo de los archivos.
/// - `header`: Define el componente de la barra de navegación superior.
/// - `file_editor`: Define el componente de edición de archivos.
pub mod components;

pub mod node;

pub mod client;
