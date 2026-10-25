//! Small helpers: JSON access in the loose, Python-like style MCP payloads need, time, randomness, logging.
use serde_json::{Map, Value};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Obj = Map<String, Value>;

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn log_err(msg: &str) {
    eprintln!("[gui-gate] {msg}");
}

/// Random hex string (confirmation codes, session ids). std only: RandomState is seeded from the OS.
pub fn rand_hex(n_bytes: usize) -> String {
    let mut out = String::new();
    while out.len() < n_bytes * 2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out.truncate(n_bytes * 2);
    out
}

/// Text of an MCP tool result: its text content parts joined by newlines.
pub fn text_of(result: &Value) -> String {
    let mut parts = vec![];
    if let Some(arr) = result.get("content").and_then(|c| c.as_array()) {
        for c in arr {
            if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                parts.push(c.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string());
            }
        }
    }
    parts.join("\n")
}

pub fn is_error(v: &Value) -> bool {
    truthy(v.get("isError"))
}

/// Python-style truthiness of an optional JSON value.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

pub fn present(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null))
}

/// An integer from a JSON number or numeric string.
pub fn as_i64(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

pub fn as_f64(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// str(x) the way Python would show a scalar (for building messages and signatures).
pub fn pystr(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => if *b { "True".into() } else { "False".into() },
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
    }
}

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("")
}

pub fn sc(result: &Value) -> Value {
    result.get("structuredContent").cloned().filter(|v| v.is_object()).unwrap_or(Value::Object(Obj::new()))
}

pub fn push_text(res: &mut Value, text: String) {
    if !res.is_object() {
        *res = Value::Object(Obj::new());
    }
    let o = res.as_object_mut().unwrap();
    let arr = o.entry("content").or_insert_with(|| Value::Array(vec![]));
    if !arr.is_array() {
        *arr = Value::Array(vec![]);
    }
    arr.as_array_mut().unwrap().push(serde_json::json!({"type": "text", "text": text}));
}

pub fn text_result(text: impl Into<String>, err: bool) -> Value {
    let mut v = serde_json::json!({"content": [{"type": "text", "text": text.into()}]});
    if err {
        v["isError"] = Value::Bool(true);
    }
    v
}

pub fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Last n chars.
pub fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

pub fn round_to(x: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (x * m).round() / m
}

/// Python's round(x, -1) style rounding used in pixel signatures (banker's rounding is irrelevant here).
pub fn round_tens(x: f64) -> f64 {
    (x / 10.0).round() * 10.0
}

pub fn home() -> String {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into())
}

pub fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

/// UTC civil time "YYYYmmdd-HHMMSS" without any date crate.
pub fn stamp() -> String {
    let t = now() as i64;
    let (days, rem) = (t.div_euclid(86400), t.rem_euclid(86400));
    let (h, m, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days since 1970-01-01 -> y/m/d (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}{mo:02}{d:02}-{h:02}{m:02}{sec:02}")
}

/// `word` occurs in `text` (both lowercase) bounded by non-alphanumeric characters (multi-word keys allowed).
pub fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(i) = text[start..].find(word) {
        let a = start + i;
        let b = a + word.len();
        let before = text[..a].chars().next_back().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        let after = text[b..].chars().next().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        if before && after {
            return true;
        }
        start = a + word.len().max(1);
        while start < text.len() && !text.is_char_boundary(start) {
            start += 1;
        }
    }
    false
}

/// SHA-256 (FIPS 180-4), std only: used to sign the gate's one-time MRTR request states (HMAC below).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb,
        0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f,
        0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut m = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[4 * i..4 * i + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// HMAC-SHA256 (RFC 2104), hex.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> String {
    let mut k = if key.len() > 64 { sha256(key).to_vec() } else { key.to_vec() };
    k.resize(64, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(msg);
    let ih = sha256(&inner);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&ih);
    hex(&sha256(&outer))
}

/// Replace key-shaped values (API keys, bearer tokens, long secrets after key=/token=/password=) with a marker.
/// Applied to every line the gate logs, so a key that reached a tool result or an error never reaches the log.
pub fn redact_secrets(s: &str) -> String {
    use std::sync::OnceLock;
    static R: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let rs = R.get_or_init(|| vec![
        regex::Regex::new(r"\b(sk|pk|rk)-[A-Za-z0-9_\-]{12,}").unwrap(),
        regex::Regex::new(r"\b(ghp|gho|ghs|ghu|github_pat|xox[abpr]|glpat|AKIA)[A-Za-z0-9_\-]{12,}").unwrap(),
        regex::Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9_\-\.=]{12,}").unwrap(),
        regex::Regex::new(r#"\b([A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL)[A-Z0-9_]*)(["']?\s*[=:]\s*["']?)[^\s"',;&]{6,}"#).unwrap(),
        regex::Regex::new(r#"(?i)\b(api[_-]?key|access[_-]?token|auth[_-]?token|secret[_-]?key|client[_-]?secret|password)(["']?\s*[=:]\s*["']?)[^\s"',;&]{6,}"#).unwrap(),
        regex::Regex::new(r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{5,}").unwrap(),
    ]);
    let mut out = s.to_string();
    for (i, r) in rs.iter().enumerate() {
        out = if i == 3 || i == 4 { r.replace_all(&out, "${1}${2}[redacted]").into_owned() } else { r.replace_all(&out, "[redacted]").into_owned() };
    }
    out
}

#[cfg(test)]
mod crypto_tests {
    use super::*;
    #[test]
    fn sha256_and_hmac_vectors() {
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        // RFC 4231 test case 2
        assert_eq!(hmac_sha256(b"Jefe", b"what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }
    #[test]
    fn redaction() {
        let r = redact_secrets("OPENAI_API_KEY=sk-test-ABCDEFGHIJKLMNOP1234 and Authorization: Bearer abcdefghijklmnop1234 x");
        assert!(!r.contains("ABCDEFGHIJKLMNOP1234") && !r.contains("abcdefghijklmnop1234"), "{r}");
        assert!(r.contains("OPENAI_API_KEY="));
        assert_eq!(redact_secrets("count=3 ok"), "count=3 ok");
    }
}

/// Secret values the gate holds (model keys from its environment, key files, the reviewer.env settings): every tool
/// result and every log line has them replaced by [redacted], whatever route they took (a command that printed its
/// parent's environment, a key file read by an interpreter, an error message).
fn secret_store() -> &'static std::sync::Mutex<Vec<String>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let mut v: Vec<String> = std::env::vars().filter(|(k, _)| crate::proc::is_secret_var(k) && secretish_name(k)).map(|(_, v)| v.trim().to_string()).collect();
        v.retain(|x| x.len() >= 8);
        std::sync::Mutex::new(v)
    })
}

/// A variable whose VALUE is a secret (not every GUI_GATE_ setting is: a policy name or a path is not).
fn secretish_name(k: &str) -> bool {
    let u = k.to_ascii_uppercase();
    u.ends_with("_KEY") || u.contains("TOKEN") || u.ends_with("_SECRET") || u.contains("PASSWORD") || u.contains("API_KEY")
}

pub fn register_secret(v: &str) {
    let v = v.trim();
    if v.len() >= 8 {
        let mut g = secret_store().lock().unwrap_or_else(|e| e.into_inner());
        if !g.iter().any(|x| x == v) {
            g.push(v.to_string());
        }
    }
}

/// redact_secrets plus every secret value the gate knows.
pub fn redact_all(s: &str) -> String {
    let mut out = redact_secrets(s);
    for v in secret_store().lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if out.contains(v.as_str()) {
            out = out.replace(v.as_str(), "[redacted]");
        }
    }
    out
}

/// Redact every string of a tool result (image data excluded).
pub fn redact_value(v: &mut Value) {
    match v {
        Value::String(s) => {
            let r = redact_all(s);
            if r != *s {
                *s = r;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact_value),
        Value::Object(o) => {
            let img = o.get("type").and_then(|t| t.as_str()) == Some("image");
            for (k, x) in o.iter_mut() {
                if img && k == "data" {
                    continue;
                }
                redact_value(x);
            }
        }
        _ => {}
    }
}
