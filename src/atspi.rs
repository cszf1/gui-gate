//! What the accessibility tree from Cua Driver leaves out, read straight from AT-SPI (Linux only):
//! - the visible cells of a spreadsheet grid (LibreOffice Calc's table has 2^31 children, so the driver's walk
//!   stops at it: "PARTIAL TREE ... cell grid not expanded"); read through the Table interface, visible range only;
//! - the visible text of a terminal (VTE exposes it through the Text interface; the driver lists only `terminal`).
//!
//! A minimal synchronous D-Bus client (unix socket, EXTERNAL auth) keeps the gate free of new dependencies and lets
//! one read pipeline hundreds of calls (all requests written, then all replies read): ~600 cells in one round trip
//! per stage instead of 1,800 sequential calls.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

// ------------------------------------------------------------------ values

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    Byte(u8),
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(String),
    Arr(Vec<V>),
    St(Vec<V>),
    Dict(Vec<(V, V)>),
    Var(Box<V>),
}

impl V {
    pub fn str(&self) -> Option<&str> {
        match self {
            V::Str(s) => Some(s),
            V::Var(b) => b.str(),
            _ => None,
        }
    }
    pub fn int(&self) -> Option<i64> {
        match self {
            V::Byte(x) => Some(*x as i64),
            V::I16(x) => Some(*x as i64),
            V::U16(x) => Some(*x as i64),
            V::I32(x) => Some(*x as i64),
            V::U32(x) => Some(*x as i64),
            V::I64(x) => Some(*x),
            V::U64(x) => Some(*x as i64),
            V::Var(b) => b.int(),
            _ => None,
        }
    }
    pub fn items(&self) -> &[V] {
        match self {
            V::Arr(a) | V::St(a) => a,
            V::Var(b) => b.items(),
            _ => &[],
        }
    }
    /// body of a reply: the first value
    pub fn first(&self) -> Option<&V> {
        self.items().first()
    }
    /// an (so) accessible reference
    pub fn obj(&self) -> Option<Obj> {
        let it = self.items();
        match (it.first().and_then(|v| v.str()), it.get(1).and_then(|v| v.str())) {
            (Some(a), Some(b)) => Some((a.to_string(), b.to_string())),
            _ => None,
        }
    }
}

pub type Obj = (String, String);

#[derive(Debug, Clone)]
pub enum Arg {
    S(String),
    I(i32),
    U(u32),
}

// ------------------------------------------------------------------ wire format

fn align_of(t: u8) -> usize {
    match t {
        b'y' | b'g' | b'v' => 1,
        b'n' | b'q' => 2,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        _ => 4,
    }
}

fn pad_to(buf: &mut Vec<u8>, a: usize) {
    while buf.len() % a != 0 {
        buf.push(0);
    }
}

/// end (exclusive) of the single complete type starting at sig[i]
fn type_end(sig: &[u8], i: usize) -> Result<usize, String> {
    match sig.get(i) {
        None => Err("signature ended early".into()),
        Some(b'a') => type_end(sig, i + 1),
        Some(&open @ (b'(' | b'{')) => {
            let close = if open == b'(' { b')' } else { b'}' };
            let mut j = i + 1;
            while sig.get(j) != Some(&close) {
                if j >= sig.len() {
                    return Err("unbalanced signature".into());
                }
                j = type_end(sig, j)?;
            }
            Ok(j + 1)
        }
        Some(_) => Ok(i + 1),
    }
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
    le: bool,
}

impl<'a> Rd<'a> {
    fn align(&mut self, a: usize) {
        self.p = self.p.div_ceil(a) * a;
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err("message too short".into());
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        self.align(4);
        let s = self.take(4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Ok(if self.le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) })
    }
    fn u16(&mut self) -> Result<u16, String> {
        self.align(2);
        let s = self.take(2)?;
        let a = [s[0], s[1]];
        Ok(if self.le { u16::from_le_bytes(a) } else { u16::from_be_bytes(a) })
    }
    fn u64(&mut self) -> Result<u64, String> {
        self.align(8);
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(if self.le { u64::from_le_bytes(a) } else { u64::from_be_bytes(a) })
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        let s = self.take(n)?;
        self.take(1)?;
        Ok(String::from_utf8_lossy(s).into_owned())
    }
    fn sig(&mut self) -> Result<String, String> {
        let n = self.take(1)?[0] as usize;
        let s = self.take(n)?;
        self.take(1)?;
        Ok(String::from_utf8_lossy(s).into_owned())
    }
    fn one(&mut self, t: &[u8]) -> Result<V, String> {
        Ok(match t[0] {
            b'y' => V::Byte(self.take(1)?[0]),
            b'b' => V::Bool(self.u32()? != 0),
            b'n' => V::I16(self.u16()? as i16),
            b'q' => V::U16(self.u16()?),
            b'i' => V::I32(self.u32()? as i32),
            b'u' | b'h' => V::U32(self.u32()?),
            b'x' => V::I64(self.u64()? as i64),
            b't' => V::U64(self.u64()?),
            b'd' => V::F64(f64::from_bits(self.u64()?)),
            b's' | b'o' => V::Str(self.string()?),
            b'g' => V::Str(self.sig()?),
            b'v' => {
                let s = self.sig()?;
                let sb = s.as_bytes();
                if sb.is_empty() || type_end(sb, 0)? != sb.len() {
                    return Err(format!("bad variant signature {s}"));
                }
                V::Var(Box::new(self.one(sb)?))
            }
            b'a' => {
                let n = self.u32()? as usize;
                let et = &t[1..type_end(t, 1)?];
                self.align(align_of(et[0]));
                let end = self.p + n;
                if end > self.b.len() {
                    return Err("array past end".into());
                }
                let mut items = vec![];
                let mut dict = vec![];
                while self.p < end {
                    if et[0] == b'{' {
                        self.align(8);
                        let k_end = type_end(et, 1)?;
                        let k = self.one(&et[1..k_end])?;
                        let v_end = type_end(et, k_end)?;
                        let v = self.one(&et[k_end..v_end])?;
                        dict.push((k, v));
                    } else {
                        items.push(self.one(et)?);
                    }
                }
                self.p = end;
                if et[0] == b'{' { V::Dict(dict) } else { V::Arr(items) }
            }
            b'(' | b'{' => {
                self.align(8);
                let close = if t[0] == b'(' { b')' } else { b'}' };
                let mut j = 1;
                let mut f = vec![];
                while t[j] != close {
                    let e = type_end(t, j)?;
                    f.push(self.one(&t[j..e])?);
                    j = e;
                }
                V::St(f)
            }
            c => return Err(format!("unsupported type {}", c as char)),
        })
    }
    fn all(&mut self, sig: &[u8]) -> Result<Vec<V>, String> {
        let mut out = vec![];
        let mut i = 0;
        while i < sig.len() {
            let e = type_end(sig, i)?;
            out.push(self.one(&sig[i..e])?);
            i = e;
        }
        Ok(out)
    }
}

fn put_u32(b: &mut Vec<u8>, x: u32) {
    pad_to(b, 4);
    b.extend_from_slice(&x.to_le_bytes());
}

fn put_str(b: &mut Vec<u8>, s: &str) {
    put_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
    b.push(0);
}

fn field(f: &mut Vec<u8>, code: u8, ty: u8, val: &str) {
    pad_to(f, 8);
    f.push(code);
    f.extend_from_slice(&[1, ty, 0]);
    if ty == b'g' {
        f.push(val.len() as u8);
        f.extend_from_slice(val.as_bytes());
        f.push(0);
    } else {
        put_str(f, val);
    }
}

/// a METHOD_CALL message (little-endian)
pub fn encode_call(serial: u32, dest: &str, path: &str, iface: &str, member: &str, args: &[Arg]) -> Vec<u8> {
    let mut body = vec![];
    let mut sig = String::new();
    for a in args {
        match a {
            Arg::S(s) => {
                put_str(&mut body, s);
                sig.push('s');
            }
            Arg::I(i) => {
                put_u32(&mut body, *i as u32);
                sig.push('i');
            }
            Arg::U(u) => {
                put_u32(&mut body, *u);
                sig.push('u');
            }
        }
    }
    // header fields start at offset 16 (8-aligned), so alignment relative to `f` is alignment in the message
    let mut f = vec![];
    field(&mut f, 1, b'o', path);
    field(&mut f, 6, b's', dest);
    if !iface.is_empty() {
        field(&mut f, 2, b's', iface);
    }
    field(&mut f, 3, b's', member);
    if !sig.is_empty() {
        field(&mut f, 8, b'g', &sig);
    }
    let mut m = vec![b'l', 1, 0, 1];
    m.extend_from_slice(&(body.len() as u32).to_le_bytes());
    m.extend_from_slice(&serial.to_le_bytes());
    m.extend_from_slice(&(f.len() as u32).to_le_bytes());
    m.extend_from_slice(&f);
    pad_to(&mut m, 8);
    m.extend_from_slice(&body);
    m
}

#[derive(Debug)]
pub struct Msg {
    pub kind: u8,
    pub reply_serial: u32,
    pub error: String,
    pub body: Vec<V>,
}

/// parse one complete message (header + body)
pub fn decode(buf: &[u8]) -> Result<Msg, String> {
    if buf.len() < 16 {
        return Err("short".into());
    }
    let le = buf[0] == b'l';
    let mut r = Rd { b: buf, p: 12, le };
    let fields = r.one(b"a(yv)")?;
    r.align(8);
    let (mut sig, mut reply_serial, mut error) = (String::new(), 0u32, String::new());
    for f in fields.items() {
        let it = f.items();
        let code = it.first().and_then(|v| v.int()).unwrap_or(0);
        let val = it.get(1);
        match code {
            4 => error = val.and_then(|v| v.str()).unwrap_or("").to_string(),
            5 => reply_serial = val.and_then(|v| v.int()).unwrap_or(0) as u32,
            8 => sig = val.and_then(|v| v.str()).unwrap_or("").to_string(),
            _ => {}
        }
    }
    let body = r.all(sig.as_bytes())?;
    Ok(Msg { kind: buf[1], reply_serial, error, body })
}

/// total length of the message whose fixed header is `h` (16 bytes)
fn msg_len(h: &[u8]) -> usize {
    let le = h[0] == b'l';
    let rd = |s: &[u8]| {
        let a = [s[0], s[1], s[2], s[3]];
        (if le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) }) as usize
    };
    let body = rd(&h[4..8]);
    let fields = rd(&h[12..16]);
    (16 + fields).div_ceil(8) * 8 + body
}

// ------------------------------------------------------------------ connection

pub struct Call {
    pub dest: String,
    pub path: String,
    pub iface: String,
    pub member: String,
    pub args: Vec<Arg>,
}

pub fn call(dest: &str, path: &str, iface: &str, member: &str, args: Vec<Arg>) -> Call {
    Call { dest: dest.into(), path: path.into(), iface: iface.into(), member: member.into(), args }
}

#[cfg(unix)]
pub struct Bus {
    s: UnixStream,
    serial: u32,
    deadline: Instant,
    rbuf: Vec<u8>,
}

#[cfg(unix)]
fn my_uid() -> Option<String> {
    let st = std::fs::read_to_string("/proc/self/status").ok()?;
    st.lines().find(|l| l.starts_with("Uid:"))?.split_whitespace().nth(1).map(String::from)
}

#[cfg(unix)]
fn connect_addr(addr: &str) -> Result<UnixStream, String> {
    for a in addr.split(';') {
        let Some(rest) = a.strip_prefix("unix:") else { continue };
        let kv: HashMap<&str, &str> = rest.split(',').filter_map(|p| p.split_once('=')).collect();
        if let Some(p) = kv.get("path") {
            return UnixStream::connect(p).map_err(|e| format!("connect {p}: {e}"));
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = kv.get("abstract") {
            use std::os::linux::net::SocketAddrExt;
            let sa = std::os::unix::net::SocketAddr::from_abstract_name(n.as_bytes()).map_err(|e| e.to_string())?;
            return UnixStream::connect_addr(&sa).map_err(|e| format!("connect @{n}: {e}"));
        }
    }
    Err(format!("no usable unix address in {addr:?}"))
}

#[cfg(unix)]
impl Bus {
    pub fn open(addr: &str, budget: Duration) -> Result<Bus, String> {
        let s = connect_addr(addr)?;
        let mut b = Bus { s, serial: 0, deadline: Instant::now() + budget, rbuf: vec![] };
        b.s.set_read_timeout(Some(Duration::from_millis(200))).ok();
        b.s.set_write_timeout(Some(budget)).ok();
        let uid = my_uid().ok_or("no uid")?;
        let hex: String = uid.bytes().map(|c| format!("{c:02x}")).collect();
        b.s.write_all(format!("\0AUTH EXTERNAL {hex}\r\n").as_bytes()).map_err(|e| e.to_string())?;
        let line = b.line()?;
        if !line.starts_with("OK") {
            return Err(format!("auth refused: {line}"));
        }
        b.s.write_all(b"BEGIN\r\n").map_err(|e| e.to_string())?;
        let r = b.call_many(&[call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello", vec![])]);
        r.into_iter().next().unwrap_or(Err("no hello".into()))?;
        Ok(b)
    }

    fn fill(&mut self) -> Result<(), String> {
        let mut tmp = [0u8; 65536];
        loop {
            if Instant::now() > self.deadline {
                return Err("timeout".into());
            }
            match self.s.read(&mut tmp) {
                Ok(0) => return Err("bus closed".into()),
                Ok(n) => {
                    self.rbuf.extend_from_slice(&tmp[..n]);
                    return Ok(());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    fn line(&mut self) -> Result<String, String> {
        loop {
            if let Some(i) = self.rbuf.windows(2).position(|w| w == b"\r\n") {
                let l = String::from_utf8_lossy(&self.rbuf[..i]).into_owned();
                self.rbuf.drain(..i + 2);
                return Ok(l);
            }
            self.fill()?;
        }
    }

    fn next_msg(&mut self) -> Result<Msg, String> {
        loop {
            if self.rbuf.len() >= 16 {
                let n = msg_len(&self.rbuf[..16]);
                if self.rbuf.len() >= n {
                    let m: Vec<u8> = self.rbuf.drain(..n).collect();
                    return decode(&m);
                }
            }
            self.fill()?;
        }
    }

    /// write every call, then collect the replies in order (errors per call)
    pub fn call_many(&mut self, calls: &[Call]) -> Vec<Result<V, String>> {
        let mut want: HashMap<u32, usize> = HashMap::new();
        let mut out: Vec<Result<V, String>> = (0..calls.len()).map(|_| Err("no reply".to_string())).collect();
        let mut wbuf = vec![];
        for (i, c) in calls.iter().enumerate() {
            self.serial += 1;
            want.insert(self.serial, i);
            wbuf.extend(encode_call(self.serial, &c.dest, &c.path, &c.iface, &c.member, &c.args));
        }
        if let Err(e) = self.s.write_all(&wbuf) {
            return (0..calls.len()).map(|_| Err(e.to_string())).collect();
        }
        while !want.is_empty() {
            match self.next_msg() {
                Ok(m) => {
                    if m.kind != 2 && m.kind != 3 {
                        continue;
                    }
                    if let Some(i) = want.remove(&m.reply_serial) {
                        out[i] = if m.kind == 2 { Ok(V::St(m.body)) } else { Err(m.error) };
                    }
                }
                Err(e) => {
                    for (_, i) in want.drain() {
                        out[i] = Err(e.clone());
                    }
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------ AT-SPI

const ACC: &str = "org.a11y.atspi.Accessible";
const PROPS: &str = "org.freedesktop.DBus.Properties";
pub const ROLE_FRAME: u32 = 23;
pub const ROLE_TABLE: u32 = 55;
pub const ROLE_TERMINAL: u32 = 60;
pub const ROLE_STATUS_BAR: u32 = 54;
const STATE_ACTIVE: u32 = 1;
const STATE_FOCUSED: u32 = 12;
const STATE_SHOWING: u32 = 25;

#[cfg(unix)]
pub struct Atspi {
    bus: Bus,
}

/// "AB12" -> (row 11, col 27), 0-based
pub fn parse_a1(name: &str) -> Option<(i64, i64)> {
    let n = name.trim().trim_start_matches('$');
    let letters: String = n.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = n[letters.len()..].trim_start_matches('$');
    if letters.is_empty() || letters.len() > 3 || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col = 0i64;
    for c in letters.to_ascii_uppercase().bytes() {
        col = col * 26 + (c - b'A' + 1) as i64;
    }
    Some((digits.parse::<i64>().ok()? - 1, col - 1))
}

pub fn col_name(mut c: i64) -> String {
    let mut s = vec![];
    c += 1;
    while c > 0 {
        let r = ((c - 1) % 26) as u8;
        s.push(b'A' + r);
        c = (c - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn states_has(v: &V, bit: u32) -> bool {
    let words: Vec<i64> = v.first().map(|a| a.items().iter().filter_map(|x| x.int()).collect()).unwrap_or_default();
    let (w, b) = ((bit / 32) as usize, bit % 32);
    words.get(w).map(|x| (*x as u64 >> b) & 1 == 1).unwrap_or(false)
}

fn one_line(s: &str, max: usize) -> String {
    let t: String = s.replace(['\n', '\r', '\t'], " ");
    if t.chars().count() > max { format!("{}…", t.chars().take(max).collect::<String>()) } else { t }
}

#[cfg(unix)]
impl Atspi {
    pub fn open(budget: Duration) -> Result<Atspi, String> {
        let t0 = Instant::now();
        let addr = match std::env::var("AT_SPI_BUS_ADDRESS") {
            Ok(a) if !a.is_empty() => a,
            _ => {
                let sess = std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| "no DBUS_SESSION_BUS_ADDRESS")?;
                let mut sb = Bus::open(&sess, budget)?;
                let r = sb.call_many(&[call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress", vec![])]);
                r.into_iter().next().unwrap_or(Err("no reply".into()))?.first().and_then(|v| v.str()).map(String::from).ok_or("no a11y address")?
            }
        };
        let left = budget.saturating_sub(t0.elapsed());
        Ok(Atspi { bus: Bus::open(&addr, left)? })
    }

    fn get_prop(o: &Obj, iface: &str, prop: &str) -> Call {
        call(&o.0, &o.1, PROPS, "Get", vec![Arg::S(iface.into()), Arg::S(prop.into())])
    }

    /// the application object of a process (matched by the D-Bus connection's pid)
    pub fn app_of_pid(&mut self, pid: i64) -> Option<Obj> {
        let r = self.bus.call_many(&[call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root", ACC, "GetChildren", vec![])]);
        let apps: Vec<Obj> = r.into_iter().next()?.ok()?.first()?.items().iter().filter_map(|v| v.obj()).collect();
        let calls: Vec<Call> = apps
            .iter()
            .map(|a| call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID", vec![Arg::S(a.0.clone())]))
            .collect();
        let pids = self.bus.call_many(&calls);
        apps.into_iter().zip(pids).find(|(_, p)| p.as_ref().ok().and_then(|v| v.first()).and_then(|v| v.int()) == Some(pid)).map(|(a, _)| a)
    }

    /// the app's top-level windows (frame/dialog...) with name and whether active
    pub fn windows(&mut self, app: &Obj) -> Vec<(Obj, String, bool)> {
        let r = self.bus.call_many(&[call(&app.0, &app.1, ACC, "GetChildren", vec![])]);
        let wins: Vec<Obj> = r.into_iter().next().and_then(|x| x.ok()).map(|v| v.first().map(|a| a.items().iter().filter_map(|x| x.obj()).collect()).unwrap_or_default()).unwrap_or_default();
        let mut calls = vec![];
        for w in &wins {
            calls.push(Self::get_prop(w, ACC, "Name"));
            calls.push(call(&w.0, &w.1, ACC, "GetState", vec![]));
        }
        let res = self.bus.call_many(&calls);
        wins.into_iter()
            .enumerate()
            .map(|(i, w)| {
                let name = res[2 * i].as_ref().ok().and_then(|v| v.first()).and_then(|v| v.str()).unwrap_or("").to_string();
                let active = res[2 * i + 1].as_ref().map(|v| states_has(v, STATE_ACTIVE)).unwrap_or(false);
                (w, name, active)
            })
            .collect()
    }

    /// breadth-first search for objects with one of `roles` that are showing; never lists the children of a node
    /// with more than 2,000 (a spreadsheet grid) and stops after `max_nodes` objects
    pub fn find(&mut self, root: &Obj, roles: &[u32], max_nodes: usize) -> Vec<(Obj, u32)> {
        let mut level = vec![root.clone()];
        let mut seen = 0usize;
        let mut found = vec![];
        while !level.is_empty() && seen < max_nodes {
            let root_level = seen == 0;
            seen += level.len();
            let mut calls = vec![];
            for o in &level {
                calls.push(call(&o.0, &o.1, ACC, "GetRole", vec![]));
                calls.push(Self::get_prop(o, ACC, "ChildCount"));
                calls.push(call(&o.0, &o.1, ACC, "GetState", vec![]));
            }
            let res = self.bus.call_many(&calls);
            let mut expand = vec![];
            for (i, o) in level.iter().enumerate() {
                let role = res[3 * i].as_ref().ok().and_then(|v| v.first()).and_then(|v| v.int()).unwrap_or(-1);
                let kids = res[3 * i + 1].as_ref().ok().and_then(|v| v.first()).and_then(|v| v.int()).unwrap_or(0);
                let showing = res[3 * i + 2].as_ref().map(|v| states_has(v, STATE_SHOWING)).unwrap_or(false);
                if role >= 0 && roles.contains(&(role as u32)) {
                    if showing {
                        found.push((o.clone(), role as u32));
                    }
                    continue;
                }
                if kids > 0 && kids <= 2000 && (showing || root_level) {
                    expand.push(o.clone());
                }
            }
            if expand.is_empty() {
                break;
            }
            let calls: Vec<Call> = expand.iter().map(|o| call(&o.0, &o.1, ACC, "GetChildren", vec![])).collect();
            let res = self.bus.call_many(&calls);
            level = res.into_iter().filter_map(|r| r.ok()).flat_map(|v| v.first().map(|a| a.items().iter().filter_map(|x| x.obj()).collect::<Vec<_>>()).unwrap_or_default()).collect();
        }
        found
    }

    fn cell_name(&mut self, cells: &[Obj]) -> Vec<String> {
        let calls: Vec<Call> = cells.iter().map(|c| Self::get_prop(c, ACC, "Name")).collect();
        self.bus.call_many(&calls).into_iter().map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.str()).map(String::from)).unwrap_or_default()).collect()
    }

    /// visible cells of a spreadsheet table: (row, col, text, formula), non-empty only, plus the visible range
    pub fn table_cells(&mut self, t: &Obj, max_rows: i64, max_cols: i64) -> Option<Grid> {
        let r = self.bus.call_many(&[call(&t.0, &t.1, "org.a11y.atspi.Component", "GetExtents", vec![Arg::U(0)]), Self::get_prop(t, ACC, "Name")]);
        let ext = r[0].as_ref().ok()?.first()?.items().iter().filter_map(|v| v.int()).collect::<Vec<_>>();
        let sheet = r[1].as_ref().ok().and_then(|v| v.first()).and_then(|v| v.str()).unwrap_or("").to_string();
        if ext.len() < 4 || ext[2] <= 6 || ext[3] <= 6 {
            return None;
        }
        let (x, y, w, h) = (ext[0] as i32, ext[1] as i32, ext[2] as i32, ext[3] as i32);
        let pts = [(x + 3, y + 3), (x + w - 3, y + h - 3)];
        let calls: Vec<Call> = pts.iter().map(|(px, py)| call(&t.0, &t.1, "org.a11y.atspi.Component", "GetAccessibleAtPoint", vec![Arg::I(*px), Arg::I(*py), Arg::U(0)])).collect();
        let corners: Vec<Obj> = self.bus.call_many(&calls).into_iter().filter_map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.obj()))).collect();
        if corners.len() < 2 {
            return None;
        }
        let names = self.cell_name(&corners);
        let (tl, br) = match (parse_a1(&names[0]), parse_a1(&names[1])) {
            (Some(a), Some(b)) => (a, b),
            _ => {
                // not Calc-style names: use the Table interface's index mapping
                let calls: Vec<Call> = corners.iter().map(|c| call(&c.0, &c.1, ACC, "GetIndexInParent", vec![])).collect();
                let idx: Vec<i64> = self.bus.call_many(&calls).into_iter().map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.int())).unwrap_or(-1)).collect();
                if idx.iter().any(|i| *i < 0) {
                    return None;
                }
                let mut calls = vec![];
                for i in &idx {
                    calls.push(call(&t.0, &t.1, "org.a11y.atspi.Table", "GetRowAtIndex", vec![Arg::I(*i as i32)]));
                    calls.push(call(&t.0, &t.1, "org.a11y.atspi.Table", "GetColumnAtIndex", vec![Arg::I(*i as i32)]));
                }
                let rc: Vec<i64> = self.bus.call_many(&calls).into_iter().map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.int())).unwrap_or(-1)).collect();
                if rc.iter().any(|v| *v < 0) {
                    return None;
                }
                ((rc[0], rc[1]), (rc[2], rc[3]))
            }
        };
        let (r0, c0) = tl;
        let r1 = br.0.max(r0).min(r0 + max_rows - 1);
        let c1 = br.1.max(c0).min(c0 + max_cols - 1);
        let mut pos = vec![];
        let mut calls = vec![];
        for r in r0..=r1 {
            for c in c0..=c1 {
                pos.push((r, c));
                calls.push(call(&t.0, &t.1, "org.a11y.atspi.Table", "GetAccessibleAt", vec![Arg::I(r as i32), Arg::I(c as i32)]));
            }
        }
        let cells: Vec<Option<Obj>> = self.bus.call_many(&calls).into_iter().map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.obj()))).collect();
        let calls: Vec<Call> = cells.iter().flatten().map(|c| call(&c.0, &c.1, "org.a11y.atspi.Text", "GetText", vec![Arg::I(0), Arg::I(-1)])).collect();
        let mut texts = self.bus.call_many(&calls).into_iter().map(|r| r.ok().and_then(|v| v.first().and_then(|x| x.str()).map(String::from)).unwrap_or_default());
        let mut filled: Vec<(i64, i64, String, Obj)> = vec![];
        for (p, c) in pos.iter().zip(cells.iter()) {
            if let Some(c) = c {
                let tx = texts.next().unwrap_or_default();
                if !tx.trim().is_empty() {
                    filled.push((p.0, p.1, tx, c.clone()));
                }
            }
        }
        let calls: Vec<Call> = cells.iter().flatten().map(|c| call(&c.0, &c.1, ACC, "GetState", vec![])).collect();
        let states = self.bus.call_many(&calls);
        let mut cursor = None;
        let mut k = 0usize;
        for (p, c) in pos.iter().zip(cells.iter()) {
            if c.is_some() {
                if states.get(k).and_then(|r| r.as_ref().ok()).map(|v| states_has(v, STATE_FOCUSED)).unwrap_or(false) {
                    cursor = Some(*p);
                }
                k += 1;
            }
        }
        let calls: Vec<Call> = filled.iter().map(|f| call(&f.3 .0, &f.3 .1, ACC, "GetAttributes", vec![])).collect();
        let attrs = self.bus.call_many(&calls);
        let mut out = vec![];
        for (f, a) in filled.into_iter().zip(attrs) {
            let formula = a
                .ok()
                .and_then(|v| match v.first() {
                    Some(V::Dict(d)) => d.iter().find(|(k, _)| k.str() == Some("Formula")).and_then(|(_, v)| v.str()).map(String::from),
                    _ => None,
                })
                .unwrap_or_default();
            out.push(Cell { row: f.0, col: f.1, text: f.2, formula });
        }
        Some(Grid { sheet, r0, c0, r1, c1, rows_visible: br.0 - r0 + 1, cols_visible: br.1 - c0 + 1, cells: out, cursor })
    }

    /// the last `max_lines` non-blank lines of a terminal's text
    pub fn terminal_tail(&mut self, t: &Obj, max_lines: usize) -> Option<String> {
        let r = self.bus.call_many(&[Self::get_prop(t, "org.a11y.atspi.Text", "CharacterCount")]);
        let n = r.into_iter().next()?.ok()?.first()?.int()? as i32;
        let r = self.bus.call_many(&[call(&t.0, &t.1, "org.a11y.atspi.Text", "GetText", vec![Arg::I((n - 12000).max(0)), Arg::I(n)])]);
        let txt = r.into_iter().next()?.ok()?.first()?.str()?.to_string();
        Some(tail_lines(&txt, max_lines))
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub row: i64,
    pub col: i64,
    pub text: String,
    pub formula: String,
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub sheet: String,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
    pub rows_visible: i64,
    pub cols_visible: i64,
    pub cells: Vec<Cell>,
    pub cursor: Option<(i64, i64)>,
}

impl Grid {
    pub fn range(&self) -> String {
        format!("{}{}:{}{}", col_name(self.c0), self.r0 + 1, col_name(self.c1), self.r1 + 1)
    }
    /// address -> rendered item, for diffs against a later read
    pub fn items(&self) -> Vec<(String, String)> {
        self.cells.iter().map(|c| (format!("{}{}", col_name(c.col), c.row + 1), cell_item(c))).collect()
    }
    fn cursor_note(&self) -> String {
        self.cursor.map(|(r, c)| format!(" Cursor (active cell): {}{}.", col_name(c), r + 1)).unwrap_or_default()
    }
}

fn cell_item(c: &Cell) -> String {
    let mut item = format!("{}{}={}", col_name(c.col), c.row + 1, one_line(&c.text, 60));
    if !c.formula.is_empty() && c.formula != c.text {
        let f = if c.formula.starts_with('=') { c.formula.clone() } else { format!("={}", c.formula) };
        item.push_str(&format!(" {{{}}}", one_line(&f, 80)));
    }
    item
}

/// what changed against the previous read of the same window: None when the visible range moved (send it all)
pub fn render_grid_diff(prev_range: &str, prev: &[(String, String)], g: &Grid, max_chars: usize) -> Option<String> {
    if prev_range != g.range() {
        return None;
    }
    let old: HashMap<&str, &str> = prev.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let now = g.items();
    let now_keys: std::collections::HashSet<&str> = now.iter().map(|(a, _)| a.as_str()).collect();
    let mut ch: Vec<String> = now.iter().filter(|(a, b)| old.get(a.as_str()) != Some(&b.as_str())).map(|(_, b)| b.clone()).collect();
    ch.extend(prev.iter().filter(|(a, _)| !now_keys.contains(a.as_str())).map(|(a, _)| format!("{a}=(now empty)")));
    if ch.is_empty() {
        return Some(format!("[gate] Visible cells unchanged since the last read (range {}).{}", g.range(), g.cursor_note()));
    }
    let mut s = format!("[gate] Cells changed since the last read (range {}; every other visible cell is as before):{}\n", g.range(), g.cursor_note());
    let mut n = 0;
    for c in &ch {
        if s.len() + c.len() > max_chars {
            break;
        }
        if n > 0 {
            s.push_str(" | ");
        }
        s.push_str(c);
        n += 1;
    }
    if n < ch.len() {
        s.push_str(&format!(" ... and {} more changed cells (read the window for all)", ch.len() - n));
    }
    Some(s)
}

/// last `max` lines, trailing blank lines dropped, long lines cut
pub fn tail_lines(txt: &str, max: usize) -> String {
    let lines: Vec<&str> = txt.lines().map(|l| l.trim_end()).collect();
    let end = lines.iter().rposition(|l| !l.is_empty()).map(|i| i + 1).unwrap_or(0);
    let start = end.saturating_sub(max);
    lines[start..end].iter().map(|l| one_line(l, 240)).collect::<Vec<_>>().join("\n")
}

/// compact text for the agent: one line per row with non-empty cells
pub fn render_grid(g: &Grid, max_chars: usize) -> String {
    let range = format!("{}{}:{}{}", col_name(g.c0), g.r0 + 1, col_name(g.c1), g.r1 + 1);
    let cut = if g.rows_visible > g.r1 - g.r0 + 1 || g.cols_visible > g.c1 - g.c0 + 1 {
        format!(" (the window shows {}x{} cells; the first {}x{} are listed)", g.rows_visible, g.cols_visible, g.r1 - g.r0 + 1, g.c1 - g.c0 + 1)
    } else {
        String::new()
    };
    let sheet = if g.sheet.is_empty() { String::new() } else { format!(" of \"{}\"", g.sheet) };
    let mut s = format!("[gate] Visible cells{sheet}, range {range}{cut}; empty cells omitted; a formula follows its shown value as {{=...}}.{}", g.cursor_note());
    if g.cells.is_empty() {
        s.push_str("\n(every visible cell is empty)");
        return s;
    }
    let mut row = -1;
    let mut more = 0usize;
    for c in &g.cells {
        let item = cell_item(c);
        if s.len() + item.len() + 8 > max_chars {
            more += 1;
            continue;
        }
        if c.row != row {
            s.push_str(&format!("\n{}: ", c.row + 1));
            row = c.row;
        } else {
            s.push_str(" | ");
        }
        s.push_str(&item);
    }
    if more > 0 {
        s.push_str(&format!("\n... {more} more non-empty cells not listed (scroll or move the cursor to bring them into view)"));
    }
    s
}

// ------------------------------------------------------------------ entry points for the server

pub fn cells_enabled() -> bool {
    std::env::var("GUI_GATE_CELLS").map(|v| v != "0").unwrap_or(true)
}

pub fn term_text_enabled() -> bool {
    std::env::var("GUI_GATE_TERM_TEXT").map(|v| v != "0").unwrap_or(true)
}

/// pick the target window's objects: the frame named like the window title, else the active one, else all
#[cfg(unix)]
fn scoped(a: &mut Atspi, app: &Obj, title: &str, roles: &[u32], max_nodes: usize) -> Vec<(Obj, u32)> {
    let wins = a.windows(app);
    let pick: Vec<&(Obj, String, bool)> = {
        let by_title: Vec<_> = wins.iter().filter(|w| !title.is_empty() && w.1 == title).collect();
        if !by_title.is_empty() {
            by_title
        } else {
            let act: Vec<_> = wins.iter().filter(|w| w.2).collect();
            if !act.is_empty() { act } else { wins.iter().collect() }
        }
    };
    let mut out = vec![];
    for w in pick {
        out.extend(a.find(&w.0, roles, max_nodes));
        if !out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(unix)]
pub fn visible_grid(pid: i64, title: &str, budget: Duration) -> Result<Grid, String> {
    let mut a = Atspi::open(budget)?;
    let app = a.app_of_pid(pid).ok_or("app not on the accessibility bus")?;
    let tables = scoped(&mut a, &app, title, &[ROLE_TABLE], 4000);
    // the sheet grid is the big table (other tables: e.g. a dialog preview)
    for (t, _) in tables {
        let r = a.bus.call_many(&[Atspi::get_prop(&t, "org.a11y.atspi.Table", "NRows")]);
        let rows = r.into_iter().next().and_then(|x| x.ok()).and_then(|v| v.first().and_then(|x| x.int())).unwrap_or(0);
        if rows < 1000 {
            continue;
        }
        if let Some(g) = a.table_cells(&t, 40, 16) {
            return Ok(g);
        }
    }
    Err("no visible spreadsheet grid".into())
}

/// "[gate] Visible cells ..." for the spreadsheet grid shown in pid's window
pub fn visible_cells(pid: i64, title: &str, budget: Duration) -> Result<String, String> {
    visible_grid(pid, title, budget).map(|g| render_grid(&g, 6000))
}

#[cfg(unix)]
pub fn terminal_text(pid: i64, title: &str, budget: Duration) -> Result<String, String> {
    let mut a = Atspi::open(budget)?;
    let app = a.app_of_pid(pid).ok_or("app not on the accessibility bus")?;
    let terms = scoped(&mut a, &app, title, &[ROLE_TERMINAL], 600);
    let (t, _) = terms.into_iter().next().ok_or("no visible terminal")?;
    let tail = a.terminal_tail(&t, 30).ok_or("terminal text not readable")?;
    Ok(if tail.trim().is_empty() {
        "[gate] Terminal text (visible screen): (empty)".to_string()
    } else {
        format!("[gate] Terminal text (last lines of the visible screen; newest at the bottom):\n{tail}")
    })
}

/// Name and description of every child of the status bar(s) in pid's window (LibreOffice puts its "document
/// modified" indicator there).
#[cfg(unix)]
pub fn status_bar_texts(pid: i64, title: &str, budget: Duration) -> Result<Vec<String>, String> {
    let mut a = Atspi::open(budget)?;
    let app = a.app_of_pid(pid).ok_or("app not on the accessibility bus")?;
    let bars = scoped(&mut a, &app, title, &[ROLE_STATUS_BAR], 3000);
    let mut out = vec![];
    for (b, _) in bars.iter().take(2) {
        let r = a.bus.call_many(&[call(&b.0, &b.1, ACC, "GetChildren", vec![])]);
        let kids: Vec<Obj> = r.into_iter().next().and_then(|x| x.ok()).map(|v| v.first().map(|a| a.items().iter().filter_map(|x| x.obj()).collect()).unwrap_or_default()).unwrap_or_default();
        let mut calls = vec![];
        // LibreOffice 25.2 (checked on gui-pi2 2026-10-22): the items are labels with empty Name/Description whose
        // shown text ("Sheet 1 of 1", "English (USA)", "100%") is only in the Text interface; the "document modified"
        // indicator is an image with no text at all, so a modified flag cannot be read here (status_says_modified ->
        // None). Read all three so the evidence is what the bar shows.
        for k in kids.iter().take(40) {
            calls.push(Atspi::get_prop(k, ACC, "Name"));
            calls.push(Atspi::get_prop(k, ACC, "Description"));
            calls.push(call(&k.0, &k.1, "org.a11y.atspi.Text", "GetText", vec![Arg::I(0), Arg::I(-1)]));
        }
        for r in a.bus.call_many(&calls) {
            if let Some(t) = r.ok().and_then(|v| v.first().and_then(|x| x.str()).map(String::from)).filter(|t| !t.trim().is_empty()) {
                let t = one_line(&t, 160);
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
    }
    Ok(out)
}

/// The title of the window that has the ACTIVE state on the accessibility bus, if any.
#[cfg(unix)]
pub fn active_window(budget: Duration) -> Result<Option<String>, String> {
    let mut a = Atspi::open(budget)?;
    let r = a.bus.call_many(&[call("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root", ACC, "GetChildren", vec![])]);
    let apps: Vec<Obj> = r.into_iter().next().and_then(|x| x.ok()).and_then(|v| v.first().map(|a| a.items().iter().filter_map(|v| v.obj()).collect())).unwrap_or_default();
    for app in apps.iter().take(60) {
        // xfwm4 exposes its own unnamed "window" with the ACTIVE state (checked on gui-pi2 2026-10-22: it came first
        // and hid the real active frame), so an active window must have a name
        if let Some((_, name, _)) = a.windows(app).into_iter().find(|w| w.2 && !w.1.trim().is_empty()) {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

#[cfg(not(unix))]
pub fn status_bar_texts(_pid: i64, _title: &str, _budget: Duration) -> Result<Vec<String>, String> {
    Err("AT-SPI is Linux only".into())
}

#[cfg(not(unix))]
pub fn active_window(_budget: Duration) -> Result<Option<String>, String> {
    Err("AT-SPI is Linux only".into())
}

#[cfg(not(unix))]
pub fn visible_grid(_pid: i64, _title: &str, _budget: Duration) -> Result<Grid, String> {
    Err("AT-SPI is Linux only".into())
}

#[cfg(not(unix))]
pub fn terminal_text(_pid: i64, _title: &str, _budget: Duration) -> Result<String, String> {
    Err("AT-SPI is Linux only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_names() {
        assert_eq!(parse_a1("A1"), Some((0, 0)));
        assert_eq!(parse_a1("R35"), Some((34, 17)));
        assert_eq!(parse_a1("AB12"), Some((11, 27)));
        assert_eq!(parse_a1("Standard"), None);
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(16383), "XFD");
    }

    #[test]
    fn encode_decode_roundtrip() {
        // a call encodes the header fields we decode back
        let m = encode_call(7, "org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root", ACC, "GetChildren", vec![Arg::S("x".into()), Arg::I(-1)].as_slice());
        assert_eq!(m.len(), msg_len(&m[..16]));
        let d = decode(&m).unwrap();
        assert_eq!(d.kind, 1);
        assert_eq!(d.body, vec![V::Str("x".into()), V::I32(-1)]);
    }

    #[test]
    fn decode_reply_with_structs_and_dict() {
        // hand-built METHOD_RETURN: reply_serial 7, signature "a(so)a{ss}"
        let mut body = vec![];
        put_u32(&mut body, 0); // array length placeholder
        let alen_at = body.len() - 4;
        pad_to(&mut body, 8);
        let start = body.len();
        for (a, b) in [(":1.5", "/a/1"), (":1.6", "/a/22")] {
            pad_to(&mut body, 8);
            put_str(&mut body, a);
            put_str(&mut body, b);
        }
        let n = (body.len() - start) as u32;
        body[alen_at..alen_at + 4].copy_from_slice(&n.to_le_bytes());
        put_u32(&mut body, 0);
        let dlen_at = body.len() - 4;
        pad_to(&mut body, 8);
        let ds = body.len();
        pad_to(&mut body, 8);
        put_str(&mut body, "Formula");
        put_str(&mut body, "=B3*2");
        let n = (body.len() - ds) as u32;
        body[dlen_at..dlen_at + 4].copy_from_slice(&n.to_le_bytes());
        let mut f = vec![];
        pad_to(&mut f, 8);
        f.extend_from_slice(&[5, 1, b'u', 0]);
        put_u32(&mut f, 7);
        field(&mut f, 8, b'g', "a(so)a{ss}");
        let mut m = vec![b'l', 2, 0, 1];
        m.extend_from_slice(&(body.len() as u32).to_le_bytes());
        m.extend_from_slice(&9u32.to_le_bytes());
        m.extend_from_slice(&(f.len() as u32).to_le_bytes());
        m.extend_from_slice(&f);
        pad_to(&mut m, 8);
        m.extend_from_slice(&body);
        let d = decode(&m).unwrap();
        assert_eq!(d.kind, 2);
        assert_eq!(d.reply_serial, 7);
        let objs: Vec<Obj> = d.body[0].items().iter().filter_map(|v| v.obj()).collect();
        assert_eq!(objs, vec![(":1.5".to_string(), "/a/1".to_string()), (":1.6".to_string(), "/a/22".to_string())]);
        match &d.body[1] {
            V::Dict(x) => assert_eq!(x[0].1.str(), Some("=B3*2")),
            _ => panic!(),
        }
    }

    #[test]
    fn grid_rendering() {
        let g = Grid {
            sheet: "Sheet1".into(),
            r0: 0,
            c0: 0,
            r1: 3,
            c1: 3,
            rows_visible: 35,
            cols_visible: 18,
            cells: vec![
                Cell { row: 0, col: 0, text: "Name".into(), formula: String::new() },
                Cell { row: 0, col: 1, text: "Qty".into(), formula: String::new() },
                Cell { row: 3, col: 2, text: "14".into(), formula: "B3*2".into() },
            ],
            cursor: None,
        };
        let s = render_grid(&g, 6000);
        assert!(s.contains("range A1:D4"), "{s}");
        assert!(s.contains("\n1: A1=Name | B1=Qty"), "{s}");
        assert!(s.contains("\n4: C4=14 {=B3*2}"), "{s}");
        assert!(s.contains("35x18"), "{s}");
        let mut g2 = g.clone();
        g2.cells[2].text = "16".into();
        g2.cells.remove(1);
        g2.cursor = Some((3, 2));
        let d = render_grid_diff(&g.range(), &g.items(), &g2, 4000).unwrap();
        assert!(d.contains("C4=16 {=B3*2}") && d.contains("B1=(now empty)") && d.contains("Cursor (active cell): C4"), "{d}");
        assert!(!d.contains("A1=Name"), "{d}");
        assert!(render_grid_diff(&g.range(), &g.items(), &g, 4000).unwrap().contains("unchanged"));
        let mut g3 = g.clone();
        g3.r0 = 10;
        assert!(render_grid_diff(&g.range(), &g.items(), &g3, 4000).is_none());
    }

    #[test]
    fn terminal_tail_lines() {
        let t = "a\nb\nuser@h:~$ ls\nfile1\nuser@h:~$ \n\n\n\n";
        assert_eq!(tail_lines(t, 3), "user@h:~$ ls\nfile1\nuser@h:~$");
    }
}
