use std::sync::{mpsc, Arc, Mutex};
use std::thread;

enum Message {
    NewJob(Job),
    Terminate,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Job = Box::new(f);
        if let Err(e) = self.sender.send(Message::NewJob(job)) {
            eprintln!("Error enviando tarea al thread pool: {}", e);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        println!("Enviando mensajes de terminación a los workers.");

        for _ in &self.workers {
            if let Err(e) = self.sender.send(Message::Terminate) {
                eprintln!("Error enviando mensaje de terminación: {}", e);
            }
        }

        println!("Cerrando todos los workers.");

        for worker in &mut self.workers {
            println!("Esperando a que finalice el worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                if let Err(e) = thread.join() {
                    eprintln!("Error al esperar al worker {}: {:?}", worker.id, e);
                }
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            println!("Worker {} esperando tarea...", id);
            let message = match receiver.lock() {
                Ok(guard) => guard.recv(),
                Err(e) => {
                    eprintln!("Worker {} no pudo bloquear el receiver: {}", id, e);
                    return;
                }
            };

            match message {
                Ok(Message::NewJob(job)) => {
                    println!("Worker {} recibió un trabajo, ejecutando.", id);
                    job();
                }
                Ok(Message::Terminate) => {
                    println!("Worker {} recibió orden de terminación.", id);
                    break;
                }
                Err(e) => {
                    println!("Worker {} no pudo recibir mensaje: {}", id, e);
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
