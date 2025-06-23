/* pub const ENCRYPTION_KEY: &[u8] = b"clavesupersecreta";

/// Encripta un mensaje utilizando XOR con una clave dada,
/// evitando los caracteres `\n` y `0xFF`.
///
/// # Argumentos
/// * `input` - Mensaje en bytes a encriptar.
/// * `key` - Clave en bytes.
///
/// # Retorna
/// El mensaje encriptado con un `\n` al final.
pub fn encrypt_xor(input: &[u8], key: &[u8]) -> Vec<u8> {
    let mut encrypted = Vec::new();

    for (i, &byte) in input.iter().enumerate() {
        let key_byte = key[i % key.len()];
        let xor_byte = byte ^ key_byte;

        if xor_byte == b'\n' {
            encrypted.push(0xFF);
            encrypted.push(xor_byte.wrapping_add(1));
        } else if xor_byte == 0xFF {
            encrypted.push(0xFF);
            encrypted.push(0x00);
        } else {
            encrypted.push(xor_byte);
        }
    }
    encrypted.push(b'\n');

    encrypted
}

/// Desencripta un mensaje previamente encriptado con `encrypt_xor`.
///
/// # Argumentos
/// * `encrypted` - Mensaje encriptado en bytes.
/// * `key` - La misma clave que se usó para encriptar.
///
/// # Retorna
/// Un `Vec<u8>` con el mensaje desencriptado.
pub fn decrypt_xor(encrypted: &[u8], key: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;
    let mut key_index = 0;

    while i < encrypted.len() {
        let key_byte = key[key_index % key.len()];

        if encrypted[i] == 0xFF {
            i += 1;
            if i >= encrypted.len() {
                break;
            }
            let next = encrypted[i];
            let original_byte = if next == 0x00 {
                0xFF
            } else {
                (next.wrapping_sub(1)) ^ key_byte
            };
            result.push(original_byte);
        } else {
            result.push(encrypted[i] ^ key_byte);
        }

        i += 1;
        key_index += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt() {
        let original = "holii";
        let encrypted = encrypt_xor(original.as_bytes(), ENCRYPTION_KEY);
        let decrypted = decrypt_xor(&encrypted, ENCRYPTION_KEY);
        assert_eq!(original.as_bytes(), &decrypted[..]);
    }

    #[test]
    fn test_encrypt_with_newline() {
        let original = "hola\ncomo\nestas";
        let encrypted = encrypt_xor(original.as_bytes(), ENCRYPTION_KEY);

        assert!(
            !encrypted.contains(&b'\n'),
            "Encrypted string contains a newline!"
        );

        let decrypted = decrypt_xor(&encrypted, ENCRYPTION_KEY);
        assert_eq!(original.as_bytes(), &decrypted[..]);
    }

    #[test]
    fn test_encrypt_empty_string() {
        let original = "";
        let encrypted = encrypt_xor(original.as_bytes(), ENCRYPTION_KEY);
        let decrypted = decrypt_xor(&encrypted, ENCRYPTION_KEY);
        assert_eq!(original.as_bytes(), &decrypted[..]);
    }

    #[test]
    fn test_encrypt_special_characters() {
        let original = "!@#$%^&*()_+-=[]{}|;':,./<>?";
        let encrypted = encrypt_xor(original.as_bytes(), ENCRYPTION_KEY);
        let decrypted = decrypt_xor(&encrypted, ENCRYPTION_KEY);
        assert_eq!(original.as_bytes(), &decrypted[..]);
    }
}
 */