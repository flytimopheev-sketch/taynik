//! TOTP (RFC 6238, SHA-1, шаг 30 с, 6 цифр) и base32-декодирование секрета.
//! SHA-1 и HMAC-SHA1 реализованы вручную: не тянем внешний крейт ради
//! одного алгоритма и сохраняем полностью офлайн-характер приложения.

/// SHA-1 (RFC 3174).
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// HMAC-SHA1 (RFC 2204/6238).
fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&sha1(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = Vec::with_capacity(64 + data.len());
    ipad.extend(k.iter().map(|b| b ^ 0x36));
    ipad.extend_from_slice(data);
    let inner = sha1(&ipad);
    let mut opad = Vec::with_capacity(64 + 20);
    opad.extend(k.iter().map(|b| b ^ 0x5c));
    opad.extend_from_slice(&inner);
    sha1(&opad)
}
/// Декодировать base32 (RFC 4648, регистр не важен, '=' и пробелы игнорируются).
pub fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    const ALPHA: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for c in s.chars() {
        if c == '=' || c == ' ' || c == '\t' {
            continue;
        }
        let pos = ALPHA
            .iter()
            .position(|a| (*a as char) == c.to_ascii_uppercase())
            .ok_or_else(|| format!("недопустимый символ base32: {c}"))?;
        buf = (buf << 5) | pos as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

/// TOTP-код для конкретного счётчика.
pub fn totp_at(key: &[u8], counter: u64) -> Result<String, String> {
    if key.is_empty() {
        return Err("пустой TOTP-секрет".into());
    }
    let mac = hmac_sha1(key, &counter.to_be_bytes());
    let off = (mac[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([mac[off] & 0x7f, mac[off + 1], mac[off + 2], mac[off + 3]]);
    Ok(format!("{:06}", bin % 1_000_000))
}

/// Текущий TOTP-код и число секунд до его смены.
pub fn totp_now(secret_base32: &str) -> Result<(String, u64), String> {
    let key = base32_decode(secret_base32)?;
    const STEP: u64 = 30;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let counter = secs / STEP;
    Ok((totp_at(&key, counter)?, STEP - counter % STEP))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn hmac_sha1_rfc2202() {
        let mac = hmac_sha1(&[0x0b; 20], b"Hi There");
        assert_eq!(hex(&mac), "b6173186b571428e4cb35515a4a88f6b4b0b1d2b");
    }

    #[test]
    fn base32_roundtrip() {
        assert_eq!(base32_decode("GEZDGNBVGY3TQOJQ").unwrap(), b"1234567890");
        assert_eq!(base32_decode("gezd gnbv gy3t qojq=").unwrap(), b"1234567890");
        assert!(base32_decode("abc1").is_err());
    }

    #[test]
    fn totp_rfc6238_vector_t59() {
        // Секрет ASCII "12345678901234567890" в base32; T=59 => counter=1.
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(totp_at(&key, 1).unwrap(), "287082");
        assert_eq!(totp_at(&key, 0x00000000023523EC).unwrap(), "969429");
    }
}