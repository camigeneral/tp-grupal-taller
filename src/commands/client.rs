#[derive(Debug, Clone)]
pub enum ClientCommand {
    Login { username: String, password: String },
    Logout,
    GetFiles,
    GetFile { file_id: String },
    CreateFile { file_name: String },
    DeleteFile { file_id: String },
    UpdateFile { file_id: String, content: String },
    Subscribe { file_id: String },
    Unsubscribe { file_id: String },
    Close,
}

impl ClientCommand {
    pub fn to_string(&self) -> String {
        match self {
            ClientCommand::Login { username, password } => format!("LOGIN {} {}\n", username, password),
            ClientCommand::Logout => "LOGOUT\n".to_string(),
            ClientCommand::GetFiles => "GET_FILES\n".to_string(),
            ClientCommand::GetFile { file_id } => format!("GET_FILE {}\n", file_id),
            ClientCommand::CreateFile { file_name } => format!("CREATE_FILE {}\n", file_name),
            ClientCommand::DeleteFile { file_id } => format!("DELETE_FILE {}\n", file_id),
            ClientCommand::UpdateFile { file_id, content } => format!("UPDATE_FILE {} {}\n", file_id, content),
            ClientCommand::Subscribe { file_id } => format!("SUBSCRIBE {}\n", file_id),
            ClientCommand::Unsubscribe { file_id } => format!("UNSUBSCRIBE {}\n", file_id),
            ClientCommand::Close => "CLOSE\n".to_string(),
        }
    }

    pub fn from_string(command: &str) -> Result<ClientCommand, String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        match parts[0] {
            "LOGIN" => Ok(ClientCommand::Login { username: parts[1].to_string(), password: parts[2].to_string() }),
            "LOGOUT" => Ok(ClientCommand::Logout),
            "GET_FILES" => Ok(ClientCommand::GetFiles),
            "GET_FILE" => Ok(ClientCommand::GetFile { file_id: parts[1].to_string() }),
            "CREATE_FILE" => Ok(ClientCommand::CreateFile { file_name: parts[1].to_string() }),
            "DELETE_FILE" => Ok(ClientCommand::DeleteFile { file_id: parts[1].to_string() }),
            "UPDATE_FILE" => Ok(ClientCommand::UpdateFile { file_id: parts[1].to_string(), content: parts[2].to_string() }),
            "SUBSCRIBE" => Ok(ClientCommand::Subscribe { file_id: parts[1].to_string() }),
            "UNSUBSCRIBE" => Ok(ClientCommand::Unsubscribe { file_id: parts[1].to_string() }),
            "CLOSE" => Ok(ClientCommand::Close),
            _ => Err(format!("Comando desconocido: {}", parts[0])),
        }
    }
}