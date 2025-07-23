extern crate serde_json;
extern crate curl;

use std::net::{TcpListener, TcpStream};
use curl::easy::{Easy, List};
use std::io::{BufReader,BufRead, Write};
use serde_json::json;


fn get_gemini_respond(prompt: &str) -> Vec<u8> {
    let api_key = "AIzaSyDSyVJnHxJnUXDRnM7SxphBTwEPGtOjMEI";

    let body = json!({
        "system_instruction": {
            "parts": [{
                "text": " Prompt de instrucciones para LLM usando RESP3
INSTRUCCIONES

Respondé únicamente con la respuesta solicitada. No agregues introducciones, explicaciones, comentarios, aclaraciones ni conclusiones.
No uses frases como 'Claro', 'Aquí está', 'Como modelo de lenguaje', etc.
Respondé únicamente con el texto generado.

Usá <space> para representar espacios y <enter> para representar saltos de línea.

Insertá texto solo donde se indique.

FORMATO DEL RESULTADO (RESP3)

Debés devolver la respuesta en formato RESP3, como un array de 3 elementos, con el siguiente formato exacto:
*3+LLM-RESPONSE+<nombre_archivo>+<contenido_generado>
CASOS
Si el modo de aplicación es whole-file:

Generá el contenido completo del archivo, sin importar el contenido original.

Devolvé el texto generado usando <enter> para los saltos de línea.

El resultado debe ir en el 3er campo del array.

Si el modo de aplicación es cursor, reemplazo, etc.:

Insertá el texto en la línea y offset indicados.

Si el offset está en medio de una palabra, separá la palabra e insertá tu respuesta con <space> antes y después.

Insertá tu contenido exactamente en el offset del texto ya decodificado (es decir: primero reemplazá <space> por espacio real y <enter> por \n, trabajá sobre ese texto, luego volvé a codificarlo).

REGLAS GENERALES

Nunca uses \n. Siempre usá <enter> para separar líneas.

SI TU RESPUESTA CONTIENE MUCHAS COSAS (POR EJEPLO UNA LISTA), NO ME LO SEPARES POR '\n'. QUE SE PUEDA LEER EN UNA SOLA LINEA CON read_line de RUST. DAMELO TODO JUNTO. 
Ejemplo: 
Si el prompt es 'dame 50 capitales', no me los des asi: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>
SIEMPRE ME LOS TENES QUE DAR ASI: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>

Si generás muchas líneas (por ejemplo una lista), devolvelas en una única línea usando <enter> entre ítems.

Nunca uses más de un array RESP. Todo debe estar contenido en un único array de tres elementos.

El primer elemento del array debe ser siempre +LLM-RESPONSE.

EJEMPLOS

cursor:

Prompt: archivo:'receta.txt', linea: 2, offset: 3, contenido: 'hola<space>como<space>estan', prompt: 'dame una capital', aplicacion: 'cursor'

Respuesta esperada:
*3+LLM-RESPONSE+receta.txt+hol<space>Roma<space>a<space>como<space>estan

Para whole-file:
Prompt: archivo:'receta.txt', prompt: 'generá una receta', aplicacion: 'whole-file'

Respuesta esperada:
*3+LLM-RESPONSE+receta.txt+Ingredientes:<enter>2<space>huevos<enter>100g<space>de<space>harina<enter>Instrucciones:<enter>Mezclar<space>todo.
"
            }]
        },
        "contents": [{
            "parts": [{
                "text": format!("{}", prompt)
            }]
        }]
        
    })
    .to_string();

    let mut response_data = Vec::new();
    let mut headers = List::new();
        headers.append("Content-Type: application/json").unwrap();
        headers
            .append(&format!("X-goog-api-key: {}", api_key))
            .unwrap();
    let mut easy = Easy::new();    
    easy.url("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent").unwrap();
    easy.post(true).unwrap();

    easy.http_headers(headers).unwrap();

    easy.post_fields_copy(body.as_bytes()).unwrap();

    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            response_data.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        transfer.perform().unwrap();
    }

    response_data.clone()
}

fn handle_requests(mut stream: TcpStream) {
    
    /* let input_prompt = "archivo:'receta.txt', linea: 2, offset: 5, contenido: 'hola<space>queondacomo<space>estan', prompt: 'generame una receta corta', aplicacion: 'whole-file'";
    let gemini_resp = get_gemini_respond(input_prompt.trim());
    let response_str = String::from_utf8_lossy(&gemini_resp);

    match serde_json::from_str::<serde_json::Value>(&response_str) {
        Ok(parsed) => {
            if let Some(text) = parsed["candidates"]
                .get(0)
                .and_then(|c| c["content"]["parts"].get(0))
                .and_then(|p| p["text"].as_str())
            {
                println!("Respuesta: {}", text);
                /* if let Err(e) = stream.write_all(format!("{text}\n").as_bytes()) {
                    eprintln!("Error escribiendo al cliente: {}", e);
                    
                } */
            } else {
                println!("Error: no se pudo extraer texto de Gemini");                            
            }
        }
        Err(e) => {
            println!("{}", format!("Error parseando JSON: {}\n", e));
            
        }} */

    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut input_prompt = String::new();
        match reader.read_line(&mut input_prompt) {
            Ok(0) => {
                println!("Conexión cerrada por el cliente");
                break;
            }
            Ok(_) => {
                println!("Prompt recibido: {}", input_prompt.trim());
                let gemini_resp = get_gemini_respond(input_prompt.trim());
                let response_str = String::from_utf8_lossy(&gemini_resp);

                match serde_json::from_str::<serde_json::Value>(&response_str) {
                    Ok(parsed) => {
                        if let Some(text) = parsed["candidates"]
                            .get(0)
                            .and_then(|c| c["content"]["parts"].get(0))
                            .and_then(|p| p["text"].as_str())
                        {
                            println!("Respuesta: {}", text);
                            if let Err(e) = stream.write_all(format!("{text}\n").as_bytes()) {
                                eprintln!("Error escribiendo al cliente: {}", e);
                                break;
                            }
                        } else {
                            println!("Error: no se pudo extraer texto de Gemini");                            
                        }
                    }
                    Err(e) => {
                        println!("{}", format!("Error parseando JSON: {}\n", e));
                        
                    }
                }
            }
            Err(e) => {
                eprintln!("Error leyendo del stream: {}", e);
                break;
            }
        }
    }
}

fn main() -> std::io::Result<()> {
   let listener = TcpListener::bind("127.0.0.1:4030")?;
   println!("Servidor para la llm levantado");
/*    handle_requests();*/
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Se conecto el microservicio");
                handle_requests(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
   Ok(())
}
