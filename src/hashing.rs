pub fn get_hash_slots(key: String) -> usize {
    let key_bytes = key.as_bytes();
    crc16(key_bytes) % 16384
}

fn crc16(data: &[u8]) -> usize {
    let mut crc: u16 = 0x0000;
    let poly: u16 = 0x1021;

    for &byte in data {
        crc ^= (byte as u16) << 8;

        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc <<= 1;
            }
        }
    }
    (crc & 0xFFFF) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16() {
        let data = b"123456789";
        let crc = crc16(data);
        assert_eq!(crc, 0x31C3);
    }
}
