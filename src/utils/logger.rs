use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
extern crate chrono;
use self::chrono::Local;

#[derive(Clone, Debug)]
pub struct Logger {
    sender: Sender<String>,
}

impl Logger {
    pub fn init(log_path: String, port: usize) -> Self {
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();

        // Hilo dedicado al logger
        thread::spawn(move || {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .expect("No se pudo abrir el archivo de log");

            // Solo el nodo 4000 escribe la línea de reinicio
            if port == 4000 {
                let now = Local::now().format("[%Y-%m-%d %H:%M:%S]");
                let reinicio_line = format!(
                    "\n{} -------------------- REINICIO DEL SERVIDOR --------------------\n",
                    now
                );
                let _ = file.write_all(reinicio_line.as_bytes());
            }

            for msg in rx {
                let now = Local::now().format("[%Y-%m-%d %H:%M:%S]");
                let log_line = format!("{} {}\n", now, msg);
                if let Err(e) = file.write_all(log_line.as_bytes()) {
                    eprintln!("Error escribiendo en el log: {}", e);
                }
            }
        });

        Logger { sender: tx }
    }

    pub fn log(&self, message: &str) {
        let _ = self.sender.send(message.to_string());
    }

    pub fn get_log_path_from_config(config_path: &str, key: &str) -> String {
        let config = std::fs::read_to_string(config_path).unwrap_or_default();
        for line in config.lines() {
            if let Some(path) = line.strip_prefix(key) {
                return path.trim().to_string();
            }
        }
        // Valor por defecto
        match key {
            "server_log_path=" => "server.log".to_string(),
            "microservice_log_path=" => "microservice.log".to_string(),
            _ => "server.log".to_string(),
        }
    }
}
