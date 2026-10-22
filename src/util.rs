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
