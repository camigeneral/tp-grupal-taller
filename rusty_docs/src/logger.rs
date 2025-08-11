use std::fs::{create_dir_all, OpenOptions};
use std::io::{Write, ErrorKind};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
extern crate chrono;
use self::chrono::Local;
use std::time::Duration;
use std::thread::sleep;

#[derive(Clone, Debug)]
pub struct Logger {
    sender: Sender<String>,
}

impl Logger {
    pub fn init(log_path: String, port: usize) -> Self {
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        
        // Crear directorio padre si no existe
        let log_dir = Path::new(&log_path)
            .parent()
            .expect("No se pudo obtener el directorio padre del log");
        
        if let Err(e) = create_dir_all(log_dir) {
            eprintln!("Error creando directorio de logs: {}", e);
        }

        thread::spawn(move || {
            sleep(Duration::from_millis(200));
            
            let mut file = None;
            for attempt in 1..=10 {
                match OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                {
                    Ok(f) => {
                        println!("Archivo de log abierto correctamente: {}", log_path);
                        file = Some(f);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error abriendo archivo (intento {}): {} - {}", attempt, log_path, e);
                        
                        // Intentar crear el archivo manualmente si no existe
                        if e.kind() == ErrorKind::NotFound {
                            if let Err(create_err) = std::fs::File::create(&log_path) {
                                eprintln!("Error creando archivo: {}", create_err);
                            }
                        }
                        
                        if attempt < 10 {
                            sleep(Duration::from_millis(500));
                        }
                    }
                }
            }
            
            let mut file = match file {
                Some(f) => f,
                None => {
                    eprintln!("FATAL: No se pudo abrir el archivo de log: {}", log_path);
                    return;
                }
            };

            // Solo el nodo 4000 escribe la línea de reinicio
            if port == 4000 {
                let now = Local::now().format("[%Y-%m-%d %H:%M:%S]");
                let reinicio_line = format!(
                    "\n{} -------------------- REINICIO DEL SERVIDOR --------------------\n",
                    now
                );
                let _ = file.write_all(reinicio_line.as_bytes());
                let _ = file.flush();
            }

            for msg in rx {
                let now = Local::now().format("[%Y-%m-%d %H:%M:%S]");
                let log_line = format!("{} {}\n", now, msg);
                if let Err(e) = file.write_all(log_line.as_bytes()) {
                    eprintln!("Error escribiendo en el log: {}", e);
                } else {
                    let _ = file.flush();
                }
            }
        });

        Logger { sender: tx }
    }
    pub fn log(&self, message: &str) {
        let _ = self.sender.send(message.to_string());
    }

    pub fn get_log_path_from_config(config_path: &str, key: &str) -> String {
        if let Ok(env_path) = std::env::var("LOG_FILE") {
            return env_path;
        }
        let config = std::fs::read_to_string(config_path).unwrap_or_default();
        for line in config.lines() {
            if let Some(path) = line.strip_prefix(key) {
                return path.trim().to_string();
            }
        }

        match key {
            "server_log_path=" => "/app/logs/server.log".to_string(),
            "microservice_log_path=" => "/app/logs/microservice.log".to_string(),
            "llm_microservice_path" => "/app/logs/llm_microservice.log".to_string(),
            _ => "/app/logs/server.log".to_string(),
        }       
    }
}
