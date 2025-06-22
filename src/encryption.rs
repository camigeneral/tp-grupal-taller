pub const ENCRYPTION_KEY: &[u8] = b"clavesecreta";

pub fn xor(message: &[u8], key: &[u8]) -> Vec<u8> {
    message.iter()
        .enumerate()
        .map(|(i, byte)| byte ^ key[i % key.len()])
        .collect()
}