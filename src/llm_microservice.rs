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
  - Generá el contenido completo del archivo (sin importar el contenido original).
  - No incluya `linea:<n>` en la respuesta.
  - Separá las líneas con <enter>.
  - Separá las palabras con <space>.

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

SI TU RESPUESTA CONTIENE MUCHAS COSAS (POR EJEPLO UNA LISTA), NO ME LO SEPARES POR '\n'. QUE SE PUEDA LEER EN UNA SOLA LINEA CON read_line de RUST. DAMELO TODO JUNTO. 
Ejemplo: 
Si el prompt es 'dame 50 capitales', no me los des asi: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>
SIEMPRE ME LOS TENES QUE DAR ASI: Tokio<enter>Ciudad<space>de<space>México<enter>El<space>Cairo<enter>Nueva<space>Delhi<enter>Shanghái<enter>São<space>Paulo<enter>Bombay<enter>

REGLAS SOBRE ESPACIOS

Solo usa <space> donde corresponden espacios reales en el resultado final. No agregues <space> extra al inicio o final a menos que el contenido generado realmente requiera espacios en esas posiciones.

Si el offset está en medio de una palabra, la palabra debe dividirse, y el contenido generado debe insertarse con un <space> antes y después:

Ejemplo: ho<space>Siam<space>la

Si el offset está en un límite claro de palabra (entre dos <space>), entonces:

Insertar directamente: <space>NUEVO<space>.

Si el contenido generado contiene múltiples palabras, todas deben estar separadas por <space> y no debe haber dobles <space> ni <space> mal ubicados.

EJEMPLOS
▸ whole-file:  
Prompt: archivo:'receta.txt', prompt: 'generá una receta', aplicacion: 'whole-file'  
Respuesta esperada:  
llm-response receta.txt Ingredientes:<enter>2<space>huevos<enter>100g<space>de<space>harina<enter>Instrucciones:<enter>Mezclar<space>todo.

▸ cursor:  
Prompt: archivo:'receta.txt', linea: 0, offset: 3, contenido: 'hola<space>como<space>estan', prompt: 'dame una capital', aplicacion: 'cursor'  
Respuesta esperada:  
llm-response receta.txt linea:0 hol<space>Roma<space>a<space>como<space>estan

Prompt: archivo:'receta.txt', linea: 0, offset: 2, contenido: 'hola<space>como<space>estan', prompt: 'lorem de dos palabras', aplicacion: 'cursor'  
Respuesta esperada:  
llm-response receta.txt linea:0 ho<space>Lorem<space>ipsum<space>la<space>como<space>estan

▸ cursor:
Prompt: archivo:'saludo.txt', linea: 0, offset: 4, contenido: 'hola<space>mundo', prompt: 'insertar palabra sorpresa', aplicacion: 'cursor'
Respuesta esperada:
llm-response saludo.txt linea:0 hola<space>sorpresa<space>mundo

Prompt: archivo:'saludo.txt', linea: 0, offset: 2, contenido: 'hola<space>mundo', prompt: 'insertar un número romano', aplicacion: 'cursor'
Respuesta esperada:
llm-response saludo.txt linea:0 ho<space>IV<space>la<space>mundo

Prompt: archivo:'gatos.txt', linea: 1, offset: 7, contenido: 'Maine<space>coon<space>gato', prompt: 'añadir tipo de gato', aplicacion: 'cursor'
Respuesta esperada:
llm-response gatos.txt linea:1 Maine<space>co<space>Siamés<space>on<space>gato

Prompt: archivo:'colores.txt', linea: 2, offset: 10, contenido: 'rojo<space>verde<space>azul', prompt: 'agregar un color primario', aplicacion: 'cursor'
Respuesta esperada:
llm-response colores.txt linea:2 rojo<space>verde<space>az<space>amarillo<space>ul

Prompt: archivo:'test.txt', linea: 0, offset: 0, contenido: 'hola<space>como', prompt: 'insertar saludo inicial', aplicacion: 'cursor'
Respuesta esperada:
llm-response test.txt linea:0 <space>Hola<space>hola<space>como

▸ whole-file:
Prompt: archivo:'planetas.txt', prompt: 'dame los 4 primeros planetas', aplicacion: 'whole-file'
Respuesta esperada:
llm-response planetas.txt Mercurio<enter>Venus<enter>Tierra<enter>Marte

Prompt: archivo:'razas.txt', prompt: 'tres razas de perro', aplicacion: 'whole-file'
Respuesta esperada:
llm-response razas.txt Labrador<space>Retriever<enter>Pastor<space>Alemán<enter>Bulldog<enter>

Prompt: archivo:'frutas.txt', prompt: 'nombres de frutas', aplicacion: 'whole-file'
Respuesta esperada:
llm-response frutas.txt Manzana<enter>Pera<enter>Banana<enter>Frutilla<enter>Durazno

Prompt: archivo:'notas.txt', prompt: 'lista de 7 notas musicales', aplicacion: 'whole-file'
Respuesta esperada:
llm-response notas.txt Do<enter>Re<enter>Mi<enter>Fa<enter>Sol<enter>La<enter>Si

▸ cursor con offset dentro de palabra:
Prompt: archivo:'texto.txt', linea: 0, offset: 5, contenido: 'gente<space>linda', prompt: 'insertar una palabra', aplicacion: 'cursor'
Respuesta esperada:
llm-response texto.txt linea:0 gente<space>bella<space>linda

Prompt: archivo:'ideas.txt', linea: 0, offset: 8, contenido: 'gran<space>proyecto<space>final', prompt: 'inserta una palabra clave', aplicacion: 'cursor'
Respuesta esperada:
llm-response ideas.txt linea:0 gran<space>proye<space>clave<space>cto<space>final

▸ cursor con palabras múltiples:
Prompt: archivo:'data.txt', linea: 0, offset: 4, contenido: 'hola<space>como<space>va', prompt: 'dos nombres', aplicacion: 'cursor'
Respuesta esperada:
llm-response data.txt linea:0 hola<space>Juan<space>Ana<space>como<space>va

▸ cursor con inserción al final:
Prompt: archivo:'mensaje.txt', linea: 0, offset: 13, contenido: 'esto<space>es<space>un<space>mensaje', prompt: 'añadir palabra final', aplicacion: 'cursor'
Respuesta esperada:
llm-response mensaje.txt linea:0 esto<space>es<space>un<space>mensaje<space>final

▸ cursor con inserción al principio:
Prompt: archivo:'salida.txt', linea: 0, offset: 0, contenido: 'buen<space>día', prompt: 'insertar saludo', aplicacion: 'cursor'
Respuesta esperada:
llm-response salida.txt linea:0 <space>Hola<space>buen<space>día
"
            }]
        },
        "contents": [{
            "parts": [{
                "text": format!("{}", prompt.to_string())
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
                if input_prompt.trim().is_empty() {
                    println!("Prompt vacio: {}", input_prompt.trim());
                    break;
                }
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
                            let resp = text.trim().trim_end_matches("\n");
                            if let Err(e) = stream.write_all(format!("{resp}\n").as_bytes()) {
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
