/// Módulo que maneja la edición de archivos.
///
/// Este módulo define el componente `FileEditorModel`, que se encarga de la
/// visualización y edición de archivos. Permite a los usuarios editar el contenido
/// de los archivos, guardar cambios y regresar a la pantalla anterior.
///
///
/// Estructuras:
/// - `FileEditorModel`: Modelo que representa el estado de un editor de archivo.
/// - `FileEditorMessage`: Enum que define los mensajes que afectan al editor.
/// - `FileEditorOutputMessage`: Enum que define los mensajes enviados al componente padre.
pub mod file_editor;
/// Módulo que maneja el espacio de trabajo de archivos.
///
/// Este módulo contiene el componente `FileWorkspace`, que administra las operaciones
/// sobre los archivos, como abrir, cerrar o guardar archivos. Además, maneja las interacciones
/// entre los archivos y otros componentes de la aplicación.
///
pub mod file_workspace;
/// Módulo que maneja el encabezado de la aplicación.
///
/// Este módulo define el componente `Header`, que se encarga de mostrar el encabezado
/// de la interfaz de usuario con opciones como el nombre de la aplicación, la conexión y
/// botones de navegación.
pub mod header;
/// Módulo que maneja la lista de archivos.
///
/// Este módulo contiene el componente `FileListView`, que muestra una lista de archivos
/// en la interfaz de usuario y permite filtrarlos según su tipo (por ejemplo, archivos de texto
/// o hojas de cálculo). El componente también gestiona la selección de archivos.
///
pub mod list_files;
/// Módulo que define tipos comunes utilizados en la aplicación.
///
/// Este módulo define enums y structs que se utilizan a lo largo de los componentes.
/// Algunos de estos tipos incluyen `FileType`, que se usa para identificar el tipo de archivo
/// (por ejemplo, texto, hoja de cálculo, etc.).
///
pub mod types;

/// Módulo que maneja el sistema de login del usuario.
///
/// Este módulo define el formulario del ingreso, validaciones y los usuarios permitidos para el manejo de la aplicacion.
///
pub mod login;
