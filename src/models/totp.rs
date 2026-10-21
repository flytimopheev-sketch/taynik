//! TOTP (RFC 6238) и base32-декодирование секрета.
//!
//! Поддерживаются алгоритмы SHA-1 (по умолчанию) и SHA-256, а также
//! произвольное число цифр и период — как в URI-ссылках `otpauth://`,
//! которые выдают сайты двухфакторной аутентификации.
//! SHA-1 и SHA-256 реализованы вручную: не тянем внешний крейт ради
//! двух алгоритмов и сохраняем полностью офлайн-характер приложения.

use serde::{Deserialize, Serialize};

/// Алгоритм HMAC, используемый для TOTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
}

impl TotpAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            TotpAlgorithm::Sha1 => "SHA1",
            TotpAlgorithm::Sha256 => "SHA256",
        }
    }

    /// Разбор имени алгоритма из `otpauth://` (регистр не важен).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "SHA1" | "SHA-1" => Some(TotpAlgorithm::Sha1),
            "SHA256" | "SHA-256" => Some(TotpAlgorithm::Sha256),
            _ => None,
        }
    }
}

/// Параметры генерации кода: алгоритм, число цифр, период.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpParams {
    #[serde(default = "default_algorithm")]
    pub algorithm: TotpAlgorithm,
    #[serde(default = "default_digits")]
    pub digits: u8,
    #[serde(default = "default_period")]
    pub period: u32,
}

fn default_algorithm() -> TotpAlgorithm {
    TotpAlgorithm::Sha1
}
fn default_digits() -> u8 {
    6
}
fn default_period() -> u32 {
    30
}

impl Default for TotpParams {
    fn default() -> Self {
        Self {
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
        }
    }
}

impl TotpParams {
    /// Нормализовать параметры: цифры — чётное число 6 или 8, период — 1..=300.
    pub fn normalized(mut self) -> Self {
        if self.digits != 6 && self.digits != 8 {
            self.digits = 6;
        }
        if !(1..=300).contains(&self.period) {
            self.period = 30;
        }
        self
    }
}

/// SHA-1 (RFC 3174).
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
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

/// SHA-256 (FIPS 180-4).
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
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

/// HMAC-SHA256 (RFC 2104/6238).
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = Vec::with_capacity(64 + data.len());
    ipad.extend(k.iter().map(|b| b ^ 0x36));
    ipad.extend_from_slice(data);
    let inner = sha256(&ipad);
    let mut opad = Vec::with_capacity(64 + 32);
    opad.extend(k.iter().map(|b| b ^ 0x5c));
    opad.extend_from_slice(&inner);
    sha256(&opad)
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

/// TOTP-код для конкретного счётчика с заданными параметрами.
pub fn totp_at_with(key: &[u8], counter: u64, params: &TotpParams) -> Result<String, String> {
    if key.is_empty() {
        return Err("пустой TOTP-секрет".into());
    }
    let p = params.normalized();
    let mac: Vec<u8> = match p.algorithm {
        TotpAlgorithm::Sha1 => hmac_sha1(key, &counter.to_be_bytes()).to_vec(),
        TotpAlgorithm::Sha256 => hmac_sha256(key, &counter.to_be_bytes()).to_vec(),
    };
    let off = (mac[mac.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([mac[off] & 0x7f, mac[off + 1], mac[off + 2], mac[off + 3]]);
    let modulus = 10u32.pow(u32::from(p.digits));
    Ok(format!(
        "{:0width$}",
        bin % modulus,
        width = usize::from(p.digits)
    ))
}

/// Текущий TOTP-код и число секунд до его смены с заданными параметрами.
pub fn totp_now_with(secret_base32: &str, params: &TotpParams) -> Result<(String, u64), String> {
    let key = base32_decode(secret_base32)?;
    let p = params.normalized();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let period = u64::from(p.period);
    let counter = secs / period;
    Ok((totp_at_with(&key, counter, &p)?, period - counter % period))
}

/// Данные, извлечённые из ссылки `otpauth://`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpAuthUri {
    /// base32-секрет.
    pub secret: String,
    /// Издатель (обычно название сервиса).
    pub issuer: Option<String>,
    /// Учётная запись.
    pub account: Option<String>,
    /// Параметры генерации кода.
    pub params: TotpParams,
}

/// Разобрать ссылку `otpauth://totp/...`.
///
/// Поддерживаются `secret`, `issuer`, `algorithm`, `digits`, `period`.
pub fn parse_otpauth(uri: &str) -> Result<OtpAuthUri, String> {
    let uri = uri.trim();
    let rest = uri
        .strip_prefix("otpauth://")
        .ok_or_else(|| "ссылка должна начинаться с otpauth://".to_string())?;
    let rest = rest
        .strip_prefix("totp/")
        .or_else(|| rest.strip_prefix("TOTP/"))
        .ok_or_else(|| {
            "поддерживаются только ссылки otpauth://totp/ (hotp не поддерживается)".to_string()
        })?;

    let (label, query) = match rest.split_once('?') {
        Some((l, q)) => (l, q),
        None => (rest, ""),
    };

    let mut secret = None;
    let mut issuer = None;
    let mut digits = 6u8;
    let mut period = 30u32;
    let mut algorithm = TotpAlgorithm::Sha1;

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(value);
        match key.trim().to_ascii_lowercase().as_str() {
            "secret" => secret = Some(value),
            "issuer" => issuer = Some(value),
            "digits" => {
                digits = value.trim().parse::<u8>().unwrap_or(6);
            }
            "period" => {
                period = value.trim().parse::<u32>().unwrap_or(30);
            }
            "algorithm" => {
                algorithm = TotpAlgorithm::parse(&value).unwrap_or(TotpAlgorithm::Sha1);
            }
            _ => {}
        }
    }

    let secret = secret.ok_or_else(|| "в ссылке нет параметра secret".to_string())?;
    let secret = secret.replace(' ', "").to_uppercase();
    if secret.is_empty() {
        return Err("пустой секрет в ссылке".into());
    }
    // Проверяем, что секрет действительно base32.
    base32_decode(&secret).map_err(|e| format!("секрет не является base32: {e}"))?;

    // Метка: "Issuer:Account" либо просто "Account".
    let label = percent_decode(label);
    let (label_issuer, account) = match label.split_once(':') {
        Some((i, a)) => (Some(i.trim().to_string()), Some(a.trim().to_string())),
        None => (None, Some(label.trim().to_string())),
    };
    let issuer = issuer.or(label_issuer).filter(|s| !s.is_empty());
    let account = account.filter(|s| !s.is_empty());

    Ok(OtpAuthUri {
        secret,
        issuer,
        account,
        params: TotpParams {
            algorithm,
            digits,
            period,
        }
        .normalized(),
    })
}

/// Процентное декодирование строки (`%20` → пробел и т. п.).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn hmac_sha1_rfc2202() {
        // RFC 2202, test case 1: key = 0x0b x 20, data = "Hi There".
        let mac = hmac_sha1(&[0x0b; 20], b"Hi There");
        assert_eq!(hex(&mac), "b617318655057264e28bc0b6fb378c8ef146be00");
        // RFC 2202, test case 2: key = "Jefe".
        let mac = hmac_sha1(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(hex(&mac), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
        // RFC 2202, test case 6: key длиннее блока.
        let mac = hmac_sha1(
            &[0xaa; 80],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(hex(&mac), "aa4ae5e15272d00e95705637ce8a3b55ed402112");
    }

    #[test]
    fn hmac_sha256_rfc4231() {
        // RFC 4231, test case 1.
        let mac = hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(
            hex(&mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // RFC 4231, test case 2.
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn base32_roundtrip() {
        assert_eq!(base32_decode("GEZDGNBVGY3TQOJQ").unwrap(), b"1234567890");
        assert_eq!(
            base32_decode("gezd gnbv gy3t qojq=").unwrap(),
            b"1234567890"
        );
        assert!(base32_decode("abc1").is_err());
    }

    #[test]
    fn totp_rfc6238_sha1_vectors() {
        // RFC 6238, приложение B: SHA-1, секрет "12345678901234567890" (ASCII).
        // Счётчик = Unix-время / 30.
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        for (counter, expected) in [
            (1u64, "287082"),       // T = 59
            (0x23523EC, "081804"),  // T = 1111111109
            (0x23523ED, "050471"),  // T = 1111111111
            (0x273EF07, "005924"),  // T = 1234567890
            (0x3F940AA, "279037"),  // T = 2000000000
            (0x27BC86AA, "353130"), // T = 20000000000
        ] {
            assert_eq!(
                totp_at_with(&key, counter, &TotpParams::default()).unwrap(),
                expected,
                "counter = {counter:#x}"
            );
        }
    }

    #[test]
    fn totp_rfc6238_sha1_eight_digits() {
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        let p = TotpParams {
            algorithm: TotpAlgorithm::Sha1,
            digits: 8,
            period: 30,
        };
        assert_eq!(totp_at_with(&key, 1, &p).unwrap(), "94287082");
        assert_eq!(totp_at_with(&key, 0x273EF07, &p).unwrap(), "89005924");
    }

    #[test]
    fn totp_rfc6238_sha256_vectors() {
        // RFC 6238, приложение B: SHA-256, секрет "12345678901234567890123456789012".
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA").unwrap();
        let p = TotpParams {
            algorithm: TotpAlgorithm::Sha256,
            digits: 6,
            period: 30,
        };
        for (counter, expected) in [
            (1u64, "119246"),       // T = 59
            (0x23523EC, "084774"),  // T = 1111111109
            (0x23523ED, "062674"),  // T = 1111111111
            (0x273EF07, "819424"),  // T = 1234567890
            (0x3F940AA, "698825"),  // T = 2000000000
            (0x27BC86AA, "737706"), // T = 20000000000
        ] {
            assert_eq!(
                totp_at_with(&key, counter, &p).unwrap(),
                expected,
                "counter = {counter:#x}"
            );
        }
    }

    #[test]
    fn totp_supports_eight_digits() {
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        let p = TotpParams {
            algorithm: TotpAlgorithm::Sha1,
            digits: 8,
            period: 30,
        };
        assert_eq!(totp_at_with(&key, 1, &p).unwrap(), "94287082");
    }

    #[test]
    fn parse_otpauth_uri() {
        let uri = "otpauth://totp/Example%20Corp:alice@example.com\
                   ?secret=JBSWY3DPEHPK3PXP&issuer=Example%20Corp&algorithm=SHA256&digits=8&period=60";
        let parsed = parse_otpauth(uri).unwrap();
        assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(parsed.issuer.as_deref(), Some("Example Corp"));
        assert_eq!(parsed.account.as_deref(), Some("alice@example.com"));
        assert_eq!(parsed.params.algorithm, TotpAlgorithm::Sha256);
        assert_eq!(parsed.params.digits, 8);
        assert_eq!(parsed.params.period, 60);
    }

    #[test]
    fn parse_otpauth_rejects_garbage() {
        assert!(parse_otpauth("https://example.com").is_err());
        assert!(parse_otpauth("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
        assert!(parse_otpauth("otpauth://totp/x").is_err());
        assert!(parse_otpauth("otpauth://totp/x?secret=not-base32!").is_err());
    }
}
