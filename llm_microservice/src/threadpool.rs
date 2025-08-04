use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
};
use std::thread;
/// Un pool de hilos simple para ejecutar tareas concurrentemente.
///
/// # Descripción general
/// El `ThreadPool` permite ejecutar múltiples tareas (jobs) en paralelo utilizando un número fijo de hilos (workers).
/// Cada tarea es una función o closure que se envía al pool mediante el método `execute`.
/// Los workers esperan nuevas tareas en un canal y las ejecutan a medida que llegan.
///
/// # Funcionamiento interno
/// - Al crear el pool (`ThreadPool::new(size)`), se inicializan `size` workers.
/// - Cada worker es un hilo que espera tareas en un canal compartido.
/// - Cuando se llama a `execute`, la tarea se envía al canal.
/// - Un worker toma la tarea del canal y la ejecuta.
/// - Si el canal se cierra (por ejemplo, al llamar a `shutdown`), los workers terminan su bucle y el hilo se cierra.
///
/// # Diagrama ASCII
/// ```text
/// +-------------------+
/// |   ThreadPool      |
/// +-------------------+
/// | [Worker 0]        |
/// | [Worker 1]        |
/// |   ...             |
/// | [Worker N-1]      |
/// +-------------------+
///          |
///          v
///   +----------------+
///   |   Sender<Job>  |
///   +----------------+
///          |
///          v
///   +----------------+
///   |  Receiver<Job> |<---------------------------+
///   +----------------+                            |
///          |                                     |
///          v                                     |
///   +----------------+    +----------------+     |
///   |   Worker 0     |    |   Worker 1     | ... |
///   +----------------+    +----------------+     |
///   | loop {         |    | loop {         |     |
///   |   job = recv() |    |   job = recv() |     |
///   |   job()        |    |   job()        |     |
///   | }              |    | }              |     |
///   +----------------+    +----------------+     |
///          ^                                     |
///          +-------------------------------------+
/// ```
///
/// # Ejemplo de uso
/// ```text
/// let pool = ThreadPool::new(4);
/// pool.execute(|| {
///     println!("Tarea ejecutada en un hilo del pool");
/// });
/// ```
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<Sender<Job>>,
}

/// Representa un hilo trabajador dentro del pool.
///
/// Cada worker espera tareas en el canal y las ejecuta en un bucle.
/// Cuando el canal se cierra, el worker termina.
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Crea un nuevo pool de hilos con la cantidad de workers especificada.
    ///
    /// # Argumentos
    /// * `size` - Número de hilos (workers) en el pool.
    ///
    /// # Ejemplo
    /// ```text
    /// let pool = ThreadPool::new(4);
    /// ```
    pub fn new(size: usize) -> ThreadPool {
        let (sender, receiver) = channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// Envía una tarea (closure) al pool para ser ejecutada por algún worker disponible.
    ///
    /// # Argumentos
    /// * `f` - Closure o función que será ejecutada en un hilo del pool.
    ///
    /// # Ejemplo
    /// ```text
    /// pool.execute(|| {
    ///     println!("Tarea concurrente");
    /// });
    /// ```
    ///
    /// # Funcionamiento
    /// - El closure se empaqueta en un `Box<dyn FnOnce()>` y se envía por el canal.
    /// - Algún worker que esté esperando en el canal recibirá la tarea y la ejecutará.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }

    /// Finaliza el pool de hilos, esperando que todos los workers terminen.
    ///
    /// Cierra el canal de envío y espera a que cada worker termine su ejecución.
    /// Útil para una finalización ordenada del programa.
    pub fn shutdown(&mut self) {
        if let Some(sender) = self.sender.take() {
            drop(sender);
        }
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
                println!("Worker {} cerrado", worker.id);
            }
        }
        println!("Todos los workers han terminado correctamente");
    }
}

impl Worker {
    /// Crea un nuevo worker que espera tareas en el canal y las ejecuta.
    ///
    /// # Argumentos
    /// * `id` - Identificador único del worker.
    /// * `receiver` - Canal compartido desde donde recibe las tareas.
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            println!("Worker {id} Iniciado y esperando un job...");
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
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
