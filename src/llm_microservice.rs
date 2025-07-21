extern crate serde_json;
extern crate curl;

use curl::easy::{Easy, List};
use serde_json::json;

fn main() {
    let api_key = "_";

    let body = json!({
        "system_instruction": {
            "parts": [{
                "text": "Respondé únicamente con la respuesta solicitada. No agregues introducciones, explicaciones, comentarios, aclaraciones ni conclusiones. No uses frases como 'Claro', 'Aquí está', 'Como modelo de lenguaje', etc. Solo devolvé la respuesta en bruto. Nada más."
            }]
        },
        "contents": [{
            "parts": [{
                "text": ""
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

    let response_str = String::from_utf8_lossy(&response_data);
    println!("Respuesta completa:\n{}", response_str);

    // Extraer solo la respuesta de Gemini, manejando posibles errores
    match serde_json::from_str::<serde_json::Value>(&response_str) {
        Ok(parsed) => {
            if let Some(text) = parsed["candidates"]
                .get(0)
                .and_then(|c| c["content"]["parts"].get(0))
                .and_then(|p| p["text"].as_str())
            {
                println!("→ Gemini responde: {}", text);
            } else {
                println!("No se pudo extraer la respuesta de Gemini.");
            }
        }
        Err(e) => {
            println!("Error al parsear la respuesta JSON: {e}");
        }
    }
}
