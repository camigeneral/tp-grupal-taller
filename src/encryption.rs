extern crate base64;
extern crate aes;
use self::aes::Aes128;
use self::aes::cipher::{BlockEncrypt, generic_array::GenericArray};
use self::base64::{engine::general_purpose, Engine as _};

pub const KEY: [u8; 16] = *b"clavesecreta1234";
pub const ENCRYPTION: bool = false;

/// Encripta un mensaje utilizando AES-128, codificando
/// el resultado en base64 y agregando un salto de línea al final.
///
/// # Argumentos
/// * cipher - Una instancia de Aes128
/// * message - Cadena de texto a encriptar
///
/// # Retorna
/// Un String en base64 con el mensaje encriptado y con un salto de línea.
pub fn encrypt_message(
    cipher: &Aes128,
    message: &str,
) -> String {
    if !ENCRYPTION {
        return message.to_string()
    }

    let mut message_bytes = message.as_bytes().to_vec();
    let padding = 16 - (message_bytes.len() % 16);
    message_bytes.extend(vec![padding as u8; padding]);

    let mut encrypted = Vec::new();
    for chunk in message_bytes.chunks_mut(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        encrypted.extend_from_slice(&block);
    }

    let mut encoded_message = general_purpose::STANDARD.encode(&encrypted);
    encoded_message.push('\n');
    encoded_message
}


#[cfg(test)]
mod tests {
    use super::*;
    extern crate aes;
    use self::aes::Aes128;
    use self::aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};

    static KEY: &[u8; 16] = b"clavesecreta1234";

    fn create_cipher() -> Aes128 {
        let key = GenericArray::from_slice(KEY);
        Aes128::new(key)
    }

    fn decrypt_message(cipher: &Aes128, encrypted_b64: &str) -> String {
        let encrypted = general_purpose::STANDARD
            .decode(encrypted_b64.trim_end()) 
            .expect("base64 decoding failed");

        let mut decrypted = Vec::new();
        for chunk in encrypted.chunks(16) {
            let mut block = GenericArray::clone_from_slice(chunk);
            cipher.decrypt_block(&mut block);
            decrypted.extend_from_slice(&block);
        }

        if let Some(&pad_byte) = decrypted.last() {
            let pad_len = pad_byte as usize;
            let len = decrypted.len();
            decrypted.truncate(len.saturating_sub(pad_len));
        }

        String::from_utf8(decrypted).expect("invalid UTF-8")
    }

    #[test]
    fn test_encrypt_and_decrypt_simple_message() {
        let cipher = create_cipher();
        let message = "hola mundo";
        let encrypted = encrypt_message(&cipher, message);
        let decrypted = decrypt_message(&cipher, &encrypted);
        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_encrypt_and_decrypt_with_newlines() {
        let cipher = create_cipher();
        let message = "hola\nmundo\n123";
        let encrypted = encrypt_message(&cipher, message);
        let decrypted = decrypt_message(&cipher, &encrypted);
        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_encrypt_and_decrypt_empty_string() {
        let cipher = create_cipher();
        let message = "";
        let encrypted = encrypt_message(&cipher, message);
        let decrypted = decrypt_message(&cipher, &encrypted);
        assert_eq!(decrypted, message);
    }

}
