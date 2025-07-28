extern crate serde_json;
extern crate reqwest;
extern crate curl;
use std::sync::Arc;
use std::thread;
use std::net::{TcpListener, TcpStream};
use std::io::{BufReader,BufRead, Write};
use serde_json::json;
#[path = "utils/threadpool.rs"]
mod threadpool;
use threadpool::ThreadPool;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};

fn get_gemini_respond(prompt: &str) -> Result<Vec<u8>, reqwest::Error> {
    let api_key = "AIzaSyDSyVJnHxJnUXDRnM7SxphBTwEPGtOjMEI";

    let body = json!({
        "system_instruction": {
            "parts": [{
                "text": "INSTRUCCIONES

Respondé únicamente con la respuesta solicitada. No agregues introducciones, explicaciones, comentarios, aclaraciones ni conclusiones.  
No uses frases como 'Claro', 'Aquí está', 'Como modelo de lenguaje', etc.  
Respondé únicamente con el texto generado.

Usá <space> para representar espacios y <enter> para representar saltos de línea.

Insertá texto solo donde se indique.

IMPORTANTE SOBRE OFFSET: El offset se calcula sobre el contenido DECODIFICADO (después de reemplazar <space> con espacios reales, <enter> con \n, etc.)

Ejemplo de cálculo de offset:
Contenido codificado: 'hola<space>mundo'
Contenido decodificado: 'hola mundo' (10 caracteres)
- offset 0 = antes de 'h'
- offset 4 = antes del espacio 
- offset 5 = antes de 'm'
- offset 10 = al final

FORMATO DEL RESULTADO

Debés devolver la respuesta como una única línea de texto, en el siguiente formato exacto:

llm-response <nombre_archivo> [linea:<n>] <contenido_codificado>

▸ Para modo whole-file:
  - Generá el contenido completo del archivo, reemplazando todo su contenido.
  - Si el prompt requiere procesar el contenido existente (traducir, reformatear, corregir, etc.), úsalo como base.
  - Si el prompt pide generar contenido nuevo desde cero (sin referencia al contenido actual), ignora el contenido original.
  - No incluya `linea:<n>` en la respuesta.
  - Separá las líneas con <enter>.
  - Separá las palabras con <space>.

Ejemplos para aclarar:

prompt: 'traduce al inglés' → Usar contenido original y traducirlo
prompt: 'dame 5 frutas' → Ignorar contenido original, generar lista nueva
prompt: 'corrige la gramática' → Usar contenido original y corregirlo
prompt: 'escribe una receta' → Ignorar contenido original, generar receta nueva

▸ Para modos como cursor, reemplazo, etc.:
  - Insertá el texto exactamente en el offset indicado, EN EL contenido original.
  - Si el offset está en medio de una palabra, separala e insertá el texto entre `<space>`.
  - Incluí la etiqueta `linea:<n>` después del nombre del archivo.
  - El contenido generado debe reflejar el texto final con la inserción aplicada.

REGLAS GENERALES

- Nunca uses \n. Siempre usá <enter> para saltos de línea.
- Devolvé todo en una única línea.
- Nunca agregues texto fuera del formato solicitado.
- No uses ningún otro delimitador más que <space> y <enter>.

SI TU RESPUESTA CONTIENE MUCHAS COSAS (POR EJEPLO UNA LISTA), NO ME LO SEPARES POR '\\n'. QUE SE PUEDA LEER EN UNA SOLA LINEA CON read_line de RUST. DAMELO TODO JUNTO. 
Ejemplo: 
Si el prompt es 'dame 50 capitales', no me los des asi: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>
SIEMPRE ME LOS TENÉS QUE DAR ASÍ: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>

REGLAS SOBRE ESPACIOS

Solo usa <space> donde corresponden espacios reales en el resultado final. No agregues <space> extra al inicio o final a menos que el contenido generado realmente requiera espacios en esas posiciones.

Si el offset está en medio de una palabra, la palabra debe dividirse, y el contenido generado debe insertarse con un <space> antes y después:

Ejemplo: ho<space>Siam<space>la

Si el offset está en un límite claro de palabra (entre dos <space>), entonces:

Insertar directamente: <space>NUEVO<space>.

Si el contenido generado contiene múltiples palabras, todas deben estar separadas por <space> y no debe haber dobles <space> ni <space> mal ubicados."
            }]
        },
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }]
    });

    let client = reqwest::blocking::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("X-goog-api-key", HeaderValue::from_str(api_key).unwrap());

    let res = client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent")
        .headers(headers)
        .json(&body)
        .send()?;

    Ok(res.bytes()?.to_vec())
}

fn handle_requests(stream: TcpStream, thread_pool: Arc<ThreadPool>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut input_prompt = String::new();
        match reader.read_line(&mut input_prompt) {
            Ok(0) => {
                println!("Conexión cerrada por el cliente");
                break;
            }
            Ok(_) => {
                let prompt = input_prompt.trim().to_string();

                if prompt.is_empty() {
                    println!("Prompt vacío");
                    break;
                }

                let mut stream_clone = stream.try_clone().unwrap();
                let prompt_clone = prompt.clone();

                thread_pool.execute(move || {
                    let gemini_resp = get_gemini_respond(&prompt_clone);

                    let response_str = match gemini_resp {
                        Ok(resp) => String::from_utf8_lossy(&resp).into_owned(),
                        Err(e) => {
                            eprintln!("Error en get_gemini_respond: {}", e);
                            return; 
                        }
                    };                

                    match serde_json::from_str::<serde_json::Value>(&response_str) {
                        Ok(parsed) => {
                            if let Some(text) = parsed["candidates"]
                                .get(0)
                                .and_then(|c| c["content"]["parts"].get(0))
                                .and_then(|p| p["text"].as_str())
                            {
                                let resp = text.trim().trim_end_matches("\n");
                                if let Err(e) = stream_clone.write_all(format!("{resp}\n").as_bytes()) {
                                    eprintln!("Error escribiendo al cliente: {}", e);
                                }
                            } else {
                                println!("Error: no se pudo extraer texto de Gemini");
                            }
                        }
                        Err(e) => {
                            println!("Error parseando JSON: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Error leyendo del stream: {}", e);
                break;
            }
        }
    }
}


fn main() -> std::io::Result<()> {
    let thread_pool = Arc::new(ThreadPool::new(4));
    let listener = TcpListener::bind("127.0.0.1:4030")?;
    
    for stream in listener.incoming() {
        let stream = stream?;
        let pool = Arc::clone(&thread_pool);
        thread::spawn(move || {
            handle_requests(stream, pool);
        });
    }
    Ok(())
}

