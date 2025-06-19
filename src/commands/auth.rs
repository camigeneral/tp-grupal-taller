use crate::commands::redis_response::RedisResponse;
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use hashing::get_hash_slots;
use client_info;


pub fn handle_auth(request: &CommandRequest, 
    logged_clients: Arc<Mutex<HashMap<String, bool>>>, 
    active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    client_addr: String,
) -> RedisResponse {
    let username = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: AUTH <username> <password>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };
    
    println!("credenciales_: {:#?}", request.arguments);
    if request.arguments.len() >= 2 || request.arguments.len() < 1  {
            println!("Cantidad de credenciales_: {:#?}", request.arguments.len());
        return RedisResponse::new(
            CommandResponse::Error("Cantidad de credenciales invalidas: AUTH <username> <password>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    } 

    let password = match request.arguments[0].clone() {
        ValueType::String(p) => {
             p.clone()
        }
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("Arguments must be strings".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if !valid_credentials(username.clone(), password) {
        return RedisResponse::new(
            CommandResponse::Error("Credenciales invalidas".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
    let mut lock_clients = active_clients.lock().unwrap();
    match lock_clients.get_mut(&client_addr) {
        Some(client) => {
            client.username = username.clone()
        }
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("Hubo probelmas al actualizar la informacion del cliente".to_string()),
                false,
                "".to_string(),
                "".to_string(),
        )
        }
    }

    let mut logged_clients_lock = logged_clients.lock().unwrap();
    println!("Intentando autenticar cliente con addr: {}", client_addr);
    println!("Formato exacto del client_addr: {:?}", client_addr);
    println!("Estado del HashMap antes de insertar: {:#?}", *logged_clients_lock);
    logged_clients_lock.insert(client_addr.clone(), true);
    println!("Cliente autenticado. Estado actual de logged_clients: {:#?}", *logged_clients_lock);
    println!("Verificando que el cliente fue insertado: {:?}", logged_clients_lock.get(&client_addr));
    
    RedisResponse::new(
        CommandResponse::Ok,
        false,       
        "".to_string(),
        "".to_string() 
    )
}

fn valid_credentials(username: String, password: String) -> bool {
    let defualt_pass = get_hash_slots("123".to_string());
    let hashed_password = get_hash_slots(password.to_string());

    let config_clients = HashMap::from([    
    ("valen".to_string(), defualt_pass),
    ("rama".to_string(), defualt_pass),
    ("cami".to_string(), defualt_pass),
    ("fran".to_string(), defualt_pass),
    ]);

    let user_password = config_clients.get(&username);
    
    if user_password.is_none() {
        return false;
    }
    
    match user_password {
        Some(hashed_pass) => {
            return *hashed_pass == hashed_password
        },
        _ => {
            return false;
        }
    }
}
