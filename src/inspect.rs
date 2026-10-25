//! 0.9.7 `inspect`: ONE read-only tool for everything the agent wants to look at besides a window tree
//! (GUI_GATE_INSPECT). Nothing here changes the machine.
//!   kind=file    read a file directly: text, PDF (pdftotext), office documents (text of the saved file), images
//!                (returned natively as an image for vision clients) - Pointer "read a file instead of writing code",
//!                OpenAPA attachments
//!   kind=env     read-only fact sheet for a feasibility check: paths the task names, apps installed, hardware,
//!                python modules, Chrome debugging port (Pointer / Intelligence-Indeed / HIPPO feasibility probing)
//!   kind=page    Chrome DevTools: open tabs and the page's text / links / form values (Pointer CDP reader; read only)
//!   kind=region  crop + zoom a part of a window screenshot (VLAA future work, UiPath crop-and-refine; vision clients)
//!   kind=locate  find the element matching a description: label match in the tree (model-free) or the grounder role
//!   kind=howto   how to do something in an app: the search role, else the built-in skills (VLAA search agent)
//!   kind=memory  experience from earlier successful runs (HIPPO / muscle-mem; GUI_GATE_MEMORY)
use crate::util::*;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::Duration;

pub const KINDS: &[&str] = &["file", "env", "page", "region", "locate", "howto", "memory"];

pub fn tool_schema() -> Value {
    json!({"name": "inspect",
        "description": "Read-only look at things other than a window tree (nothing is changed). kind=file {path}: read a file directly - text, PDF, office documents (the saved file's text), images (returned as an image) - instead of writing code to look at it. kind=env {query?}: facts for deciding whether the task is possible here (the paths the task names, installed apps, hardware, python modules). kind=page {query?}: Chrome's open tabs and the current page's text, links and form values (needs Chrome's debugging port). kind=region {pid, window_id, x, y, w, h, scale?}: zoom into part of the window's screenshot. kind=locate {pid, window_id, describe}: find the element matching a description and its element_index/token. kind=howto {query}: how to do something in an app (menu paths, shortcuts, whether the feature exists). kind=memory {query?}: action sequences that worked in earlier runs on similar tasks.",
        "inputSchema": {"type": "object", "properties": {
            "kind": {"type": "string", "enum": KINDS},
            "path": {"type": "string", "description": "kind=file: the file"},
            "query": {"type": "string", "description": "kind=howto/memory/env/page: what you want to know (page: part of a tab title or URL)"},
            "describe": {"type": "string", "description": "kind=locate: the element, e.g. 'the Bold button in the toolbar'"},
            "pid": {"type": "integer"}, "window_id": {"type": "integer"},
            "x": {"type": "number"}, "y": {"type": "number"}, "w": {"type": "number"}, "h": {"type": "number"},
            "scale": {"type": "number", "description": "kind=region: zoom factor 1-4 (default 2)"},
            "max_chars": {"type": "integer", "description": "kind=file/page: text budget (default 12000)"}},
            "required": ["kind"]}, "annotations": {"readOnlyHint": true}})
}

// ------------------------------------------------------------------ files

fn which(bin: &str) -> Option<String> {
    crate::install::which(bin)
}

pub fn ext_of(path: &str) -> String {
    crate::sidefx::splitext_ext(&crate::sidefx::basename(path)).trim_start_matches('.').to_lowercase()
}

const OFFICE: &[&str] = &["docx", "xlsx", "pptx", "odt", "ods", "odp", "xlsm", "dotx"];
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];

const OFFICE_PY: &str = r#"
import sys, zipfile, re, html
p = sys.argv[1]; ext = p.rsplit('.', 1)[-1].lower()
def txt(x):
    x = re.sub(r'<(?:w:p|a:p|text:p|text:h)[ >/][^>]*>', '\n', x); x = re.sub(r'</(?:w:p|a:p|text:p|text:h)>', '\n', x)
    x = re.sub(r'<w:tab/>|<text:tab/>', '\t', x); x = re.sub(r'<[^>]+>', '', x)
    return html.unescape(re.sub(r'\n\s*\n+', '\n', x)).strip()
out = []
if ext in ('xlsx', 'xlsm'):
    try:
        import openpyxl
        wb = openpyxl.load_workbook(p, data_only=False)
        for ws in wb.worksheets:
            out.append('## sheet %s (%s)' % (ws.title, ws.dimensions))
            for row in ws.iter_rows(max_row=200, max_col=30):
                cells = ['%s=%r' % (c.coordinate, c.value) for c in row if c.value is not None]
                if cells: out.append(' | '.join(cells))
        print('\n'.join(out)); sys.exit(0)
    except Exception as e:
        out.append('(openpyxl unavailable: %s; raw XML text follows)' % e)
z = zipfile.ZipFile(p)
names = z.namelist()
if ext in ('docx', 'dotx'): parts = ['word/document.xml'] + sorted(n for n in names if re.match(r'word/(header|footer|footnotes)\d*\.xml', n))
elif ext == 'pptx': parts = sorted((n for n in names if re.match(r'ppt/slides/slide\d+\.xml$', n)), key=lambda n: int(re.findall(r'\d+', n)[-1]))
elif ext in ('xlsx', 'xlsm'): parts = ['xl/sharedStrings.xml'] + sorted(n for n in names if n.startswith('xl/worksheets/sheet'))
else: parts = ['content.xml']
for n in parts:
    if n in names:
        out.append('## ' + n); out.append(txt(z.read(n).decode('utf8', 'replace')))
print('\n'.join(out))
"#;

/// The gate's own secrets (reviewer / role key files, reviewer.env) are never read out through inspect or a probe.
/// (Paths are compared component-wise after canonicalising, so `\\?\C:\...` on Windows and symlinks on unix match.)
pub fn secret_path(path: &str) -> bool {
    let canon = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
    let real = canon(path);
    let cfg = canon(&crate::cleanup::config_dir().to_string_lossy());
    let ext = real.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if real.starts_with(&cfg) && (ext == "key" || ext == "env") {
        return true;
    }
    if let Some(rev) = env("GUI_GATE_REVIEWER_ENV") {
        if canon(&crate::sidefx::expanduser(rev.trim())) == real {
            return true;
        }
    }
    std::env::vars().filter(|(k, _)| k.starts_with("GUI_GATE_") && k.ends_with("_KEY_FILE")).any(|(_, v)| {
        canon(&crate::sidefx::expanduser(v.trim())) == real
    })
}

/// Text of a file for the agent / the probing roles: text as is, PDF via pdftotext, office documents via the saved
/// file's XML (python3), images described (size); capped to `cap` characters.
pub fn read_file_text(path: &str, cap: usize) -> String {
    let p = std::path::Path::new(path);
    if secret_path(path) {
        return format!("{path}: refused (the gate's own key / settings file)");
    }
    let Ok(md) = std::fs::metadata(p) else { return format!("{path}: does not exist") };
    if md.is_dir() {
        let mut names: Vec<String> = std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() { format!("{n}/") } else { n }
        }).collect()).unwrap_or_default();
        names.sort();
        return format!("{path}/ is a folder with {} entries: {}", names.len(), trunc(&names.join("  "), cap));
    }
    let ext = ext_of(path);
    let body = if ext == "pdf" {
        match which("pdftotext") {
            Some(pt) => {
                let (code, out, err) = crate::proc::run_argv(&[&pt, "-layout", path, "-"], 20.0);
                if code == Some(0) { out } else { format!("(pdftotext failed: {})", trunc(&err, 200)) }
            }
            None => {
                let r = crate::proc::python().map(|py| crate::proc::run_argv(&[&py, "-c", "import sys,pypdf;print('\\n'.join(p.extract_text() or '' for p in pypdf.PdfReader(sys.argv[1]).pages))", path], 20.0));
                match r { Some((Some(0), out, _)) => out, _ => "(no PDF text extractor here: pdftotext / pypdf missing)".into() }
            }
        }
    } else if OFFICE.contains(&ext.as_str()) {
        match crate::proc::python() {
            Some(py) => {
                let (code, out, err) = crate::proc::run_argv(&[&py, "-c", OFFICE_PY, path], 30.0);
                if code == Some(0) { out } else { format!("(could not read the document: {})", trunc(&err, 300)) }
            }
            None => "(could not read the document: no python3 here)".into(),
        }
    } else if IMAGES.contains(&ext.as_str()) {
        match image::image_dimensions(p) {
            Ok((w, h)) => format!("(image {w}x{h}, {} bytes)", md.len()),
            Err(_) => format!("(image file, {} bytes)", md.len()),
        }
    } else {
        let mut buf = vec![];
        let _ = std::fs::File::open(p).and_then(|f| f.take(4 * 1024 * 1024).read_to_end(&mut buf));
        if buf.iter().take(4096).filter(|b| **b == 0).count() > 0 {
            return format!("{path}: binary file ({} bytes, type .{ext})", md.len());
        }
        String::from_utf8_lossy(&buf).to_string()
    };
    let n = body.chars().count();
    let shown: String = body.chars().take(cap).collect();
    let more = if n > cap { format!("\n[... {} more characters; ask with a larger max_chars or read a part with run_command]", n - cap) } else { String::new() };
    format!("{path} ({} bytes{}):\n{shown}{more}", md.len(), age_note(&md))
}

fn age_note(md: &std::fs::Metadata) -> String {
    md.modified().ok().and_then(|t| t.elapsed().ok()).map(|e| format!(", modified {} ago", human_secs(e.as_secs()))).unwrap_or_default()
}

pub fn human_secs(s: u64) -> String {
    if s < 90 { format!("{s} s") } else if s < 5400 { format!("{} min", s / 60) } else if s < 172800 { format!("{} h", s / 3600) } else { format!("{} days", s / 86400) }
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// kind=file as an MCP result: images come back as an image part (vision clients) plus their size.
pub fn file_result(path: &str, cap: usize, text_only: bool) -> Value {
    let path = crate::sidefx::expanduser(path.trim());
    if path.is_empty() {
        return text_result("[inspect] kind=file needs a path", true);
    }
    let txt = read_file_text(&path, cap);
    let ext = ext_of(&path);
    let mut res = text_result(txt.clone(), txt.ends_with("does not exist"));
    if IMAGES.contains(&ext.as_str()) && !text_only {
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.len() < 8 * 1024 * 1024 {
                let mime = match ext.as_str() { "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp", "bmp" => "image/bmp", _ => "image/png" };
                res["content"].as_array_mut().unwrap().push(json!({"type": "image", "data": B64.encode(&bytes), "mimeType": mime}));
            }
        }
    } else if let Some(tess) = IMAGES.contains(&ext.as_str()).then(|| which("tesseract")).flatten() {
        let (code, out, _) = crate::proc::run_argv(&[&tess, &path, "-"], 30.0);
        if code == Some(0) && !out.trim().is_empty() {
            push_text(&mut res, format!("[inspect] text in the image (OCR):\n{}", trunc(&out, cap)));
        }
    }
    res
}

// ------------------------------------------------------------------ env fact sheet

const APP_BINS: &[(&str, &[&str])] = &[
    ("chrome", &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"]), ("firefox", &["firefox"]),
    ("libreoffice", &["soffice", "libreoffice"]), ("calc", &["soffice", "localc"]), ("writer", &["soffice", "lowriter"]),
    ("impress", &["soffice", "loimpress"]), ("gimp", &["gimp"]), ("vlc", &["vlc"]), ("thunderbird", &["thunderbird"]),
    ("vs code", &["code"]), ("vscode", &["code"]), ("inkscape", &["inkscape"]), ("audacity", &["audacity"]),
    ("terminal", &["xfce4-terminal", "gnome-terminal", "xterm"]), ("python", &["python3"]), ("git", &["git"]),
    ("ffmpeg", &["ffmpeg"]), ("imagemagick", &["convert", "magick"]), ("pdf", &["pdftotext", "evince"]),
];

fn dir_nonempty(p: &str) -> bool {
    std::fs::read_dir(p).map(|mut r| r.next().is_some()).unwrap_or(false)
}

/// Read-only facts that bear on whether the task can be done here. No network, no state change.
pub fn env_facts(task: &str, query: &str) -> String {
    let mut lines: Vec<String> = vec![];
    let text = format!("{task}\n{query}");
    let low = text.to_lowercase();
    let home = home();
    // paths the task names
    let mut paths = crate::done::task_paths(&text);
    paths.truncate(12);
    for p in &paths {
        let cands: Vec<String> = if crate::sidefx::is_abs(p) || p.starts_with('~') { vec![crate::sidefx::p(p, &home)] }
            else { vec![format!("{home}/{p}"), format!("{home}/Desktop/{p}"), format!("{home}/Documents/{p}"), format!("{home}/Downloads/{p}")] };
        match cands.iter().find(|c| std::path::Path::new(c).exists()) {
            Some(c) => {
                let md = std::fs::metadata(c).ok();
                let kind = if md.as_ref().map(|m| m.is_dir()).unwrap_or(false) { "folder".to_string() } else { format!("{} bytes", md.as_ref().map(|m| m.len()).unwrap_or(0)) };
                lines.push(format!("- path \"{p}\": exists at {c} ({kind})"));
            }
            None => lines.push(format!("- path \"{p}\": NOT found (looked in {})", cands.iter().map(|c| c.replace(&home, "~")).collect::<Vec<_>>().join(", "))),
        }
    }
    // apps the task mentions
    let mut seen = std::collections::HashSet::new();
    for (word, bins) in APP_BINS {
        if !has_word(&low, word) || !seen.insert(*word) {
            continue;
        }
        match bins.iter().find_map(|b| which(b).map(|p| (b, p))) {
            Some((b, p)) => lines.push(format!("- app \"{word}\": installed ({b} at {p})")),
            None => lines.push(format!("- app \"{word}\": NOT installed (none of {})", bins.join(", "))),
        }
    }
    // hardware and devices
    let battery = std::fs::read_dir("/sys/class/power_supply").map(|rd| rd.flatten().any(|e| std::fs::read_to_string(e.path().join("type")).map(|t| t.trim() == "Battery").unwrap_or(false))).unwrap_or(false);
    let cams: Vec<String> = std::fs::read_dir("/dev").map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with("video")).collect()).unwrap_or_default();
    let sound = std::fs::read_to_string("/proc/asound/cards").map(|t| !t.contains("no soundcards") && !t.trim().is_empty()).unwrap_or(false);
    let nets: Vec<String> = std::fs::read_dir("/sys/class/net").map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n != "lo").collect()).unwrap_or_default();
    lines.push(format!("- hardware: battery {}, screen backlight control {}, camera {}, bluetooth {}, sound card {}, network interfaces {}, printers configured {}",
        yn(battery), yn(dir_nonempty("/sys/class/backlight")), if cams.is_empty() { "no".into() } else { cams.join(",") }, yn(dir_nonempty("/sys/class/bluetooth")),
        yn(sound), if nets.is_empty() { "none".into() } else { nets.join(",") }, yn(std::path::Path::new("/etc/cups/printers.conf").exists())));
    lines.push(format!("- display: DISPLAY={} home={home}", std::env::var("DISPLAY").unwrap_or_else(|_| "(unset)".into())));
    if let Some(py) = crate::proc::python() {
        let (_, out, _) = crate::proc::run_argv(&[&py, "-c", "import importlib.util as u;print(' '.join(m for m in ['openpyxl','docx','pptx','odf','pandas','PIL','pypdf','pychrome'] if u.find_spec(m)))"], 8.0);
        lines.push(format!("- python3 modules available: {}", if out.trim().is_empty() { "(none of openpyxl docx pptx odf pandas PIL pypdf)".into() } else { out.trim().to_string() }));
    }
    let port = cdp_port();
    lines.push(format!("- Chrome debugging port {port}: {}", if std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_millis(300)).is_ok() { "open (inspect kind=page works)" } else { "closed" }));
    format!("[inspect env] read-only facts (no network checks; a missing app/file/device is evidence for infeasible, not proof):\n{}", lines.join("\n"))
}

fn yn(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

// ------------------------------------------------------------------ Chrome DevTools (read only)

pub fn cdp_port() -> u16 {
    env("GUI_GATE_CDP_PORT").and_then(|p| p.trim().parse().ok()).unwrap_or(9222)
}

const PAGE_JS: &str = "JSON.stringify({title:document.title,url:location.href,text:(document.body?document.body.innerText:'').slice(0,MAXC),links:Array.from(document.links).slice(0,40).map(a=>[(a.innerText||'').trim().slice(0,60),a.href]),inputs:Array.from(document.querySelectorAll('input,select,textarea')).slice(0,30).map(e=>[e.name||e.id||e.type,e.type,e.type==='password'?'***':String(e.value||'').slice(0,80)])})";

/// Tabs from /json/list and the chosen tab's text over the DevTools websocket (a fixed read-only expression).
pub fn page_info(query: &str, max_chars: usize) -> Result<String, String> {
    let port = cdp_port();
    let list: Value = ureq::get(&format!("http://127.0.0.1:{port}/json/list")).timeout(Duration::from_secs(3)).call()
        .map_err(|e| format!("Chrome's debugging port {port} is not reachable ({e}); start Chrome with --remote-debugging-port={port} or read the page with get_window_state"))?
        .into_json().map_err(|e| e.to_string())?;
    let tabs: Vec<&Value> = list.as_array().map(|a| a.iter().filter(|t| s(t, "type") == "page").collect()).unwrap_or_default();
    let mut out = format!("[inspect page] {} tab(s):\n", tabs.len());
    for (i, t) in tabs.iter().enumerate() {
        out.push_str(&format!("{}. {} - {}\n", i + 1, trunc(s(t, "title"), 80), trunc(s(t, "url"), 160)));
    }
    let q = query.to_lowercase();
    let pick = tabs.iter().find(|t| !q.is_empty() && (s(t, "title").to_lowercase().contains(&q) || s(t, "url").to_lowercase().contains(&q))).or(tabs.first());
    let Some(tab) = pick else { return Ok(out) };
    let ws = s(tab, "webSocketDebuggerUrl");
    if ws.is_empty() {
        return Ok(out + "(the tab has no debugger URL: another DevTools client holds it)");
    }
    let expr = PAGE_JS.replace("MAXC", &max_chars.to_string());
    let v = ws_eval(ws, &expr).map_err(|e| format!("{out}(could not read the page: {e})"))?;
    let d: Value = serde_json::from_str(v.as_str().unwrap_or("{}")).unwrap_or(json!({}));
    out.push_str(&format!("\nPage \"{}\" {}\n--- text ---\n{}\n", s(&d, "title"), s(&d, "url"), s(&d, "text")));
    if let Some(l) = d.get("links").and_then(|l| l.as_array()).filter(|l| !l.is_empty()) {
        out.push_str("--- links ---\n");
        for x in l {
            out.push_str(&format!("{} -> {}\n", x[0].as_str().unwrap_or(""), x[1].as_str().unwrap_or("")));
        }
    }
    if let Some(l) = d.get("inputs").and_then(|l| l.as_array()).filter(|l| !l.is_empty()) {
        out.push_str("--- form fields ---\n");
        for x in l {
            out.push_str(&format!("{} ({}) = {}\n", x[0].as_str().unwrap_or(""), x[1].as_str().unwrap_or(""), x[2].as_str().unwrap_or("")));
        }
    }
    Ok(out)
}

/// Minimal websocket client (RFC 6455, text frames) for one Runtime.evaluate call. Returns result.result.value.
pub fn ws_eval(url: &str, expr: &str) -> Result<Value, String> {
    let rest = url.strip_prefix("ws://").ok_or("not a ws:// URL")?;
    let (hostport, path) = rest.split_once('/').map(|(h, p)| (h.to_string(), format!("/{p}"))).unwrap_or((rest.to_string(), "/".into()));
    // only the local browser's own debugging endpoint, never anything a page list could point elsewhere
    let host = hostport.rsplit_once(':').map(|x| x.0).unwrap_or(&hostport).trim_matches(['[', ']']);
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return Err(format!("refusing a non-local DevTools address {hostport}"));
    }
    let addr = std::net::ToSocketAddrs::to_socket_addrs(&hostport).map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut st = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    st.set_write_timeout(Some(Duration::from_secs(5))).ok();
    st.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let key = B64.encode(rand_hex(16).as_bytes().get(..16).unwrap_or(b"0123456789abcdef"));
    write!(st, "GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").map_err(|e| e.to_string())?;
    let mut head = vec![];
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        st.read_exact(&mut b).map_err(|e| e.to_string())?;
        head.push(b[0]);
        if head.len() > 8192 { return Err("bad handshake".into()); }
    }
    if !String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101") {
        return Err(format!("handshake refused: {}", trunc(&String::from_utf8_lossy(&head), 80)));
    }
    let msg = json!({"id": 1, "method": "Runtime.evaluate", "params": {"expression": expr, "returnByValue": true}}).to_string();
    st.write_all(&ws_frame(msg.as_bytes())).map_err(|e| e.to_string())?;
    for _ in 0..50 {
        let m = ws_read(&mut st)?;
        let v: Value = serde_json::from_slice(&m).unwrap_or(Value::Null);
        if v.get("id") == Some(&json!(1)) {
            if let Some(e) = v.get("error") {
                return Err(e.to_string());
            }
            return Ok(v["result"]["result"]["value"].clone());
        }
    }
    Err("no answer".into())
}

pub fn ws_frame(payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x81u8];
    let n = payload.len();
    if n < 126 { f.push(0x80 | n as u8); } else if n < 65536 { f.push(0x80 | 126); f.extend_from_slice(&(n as u16).to_be_bytes()); } else { f.push(0x80 | 127); f.extend_from_slice(&(n as u64).to_be_bytes()); }
    let mask = [0x12u8, 0x34, 0x56, 0x78];
    f.extend_from_slice(&mask);
    f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    f
}

fn ws_read(st: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut msg = vec![];
    loop {
        let mut h = [0u8; 2];
        st.read_exact(&mut h).map_err(|e| e.to_string())?;
        let fin = h[0] & 0x80 != 0;
        let masked = h[1] & 0x80 != 0;
        let mut n = (h[1] & 0x7f) as u64;
        if n == 126 { let mut x = [0u8; 2]; st.read_exact(&mut x).map_err(|e| e.to_string())?; n = u16::from_be_bytes(x) as u64; }
        else if n == 127 { let mut x = [0u8; 8]; st.read_exact(&mut x).map_err(|e| e.to_string())?; n = u64::from_be_bytes(x); }
        if n > 64 * 1024 * 1024 { return Err("frame too large".into()); }
        let mut mask = [0u8; 4];
        if masked { st.read_exact(&mut mask).map_err(|e| e.to_string())?; }
        let mut p = vec![0u8; n as usize];
        st.read_exact(&mut p).map_err(|e| e.to_string())?;
        if masked { for (i, b) in p.iter_mut().enumerate() { *b ^= mask[i % 4]; } }
        msg.extend(p);
        if fin { return Ok(msg); }
    }
}

// ------------------------------------------------------------------ region zoom

/// Crop (x, y, w, h) in the screenshot's pixel space and scale it up; returns PNG base64 and the crop actually used.
pub fn crop_zoom(b64: &str, x: f64, y: f64, w: f64, h: f64, scale: f64) -> Result<(String, (u32, u32, u32, u32)), String> {
    let raw = B64.decode(b64.as_bytes()).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&raw).map_err(|e| format!("cannot decode the screenshot: {e}"))?;
    let (iw, ih) = (img.width(), img.height());
    let x0 = (x.max(0.0) as u32).min(iw.saturating_sub(1));
    let y0 = (y.max(0.0) as u32).min(ih.saturating_sub(1));
    let cw = (w.max(8.0) as u32).min(iw - x0);
    let ch = (h.max(8.0) as u32).min(ih - y0);
    let sc = scale.clamp(1.0, 4.0);
    let crop = img.crop_imm(x0, y0, cw, ch);
    let out = crop.resize((cw as f64 * sc) as u32, (ch as f64 * sc) as u32, image::imageops::FilterType::Lanczos3);
    let mut buf = std::io::Cursor::new(vec![]);
    out.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok((B64.encode(buf.into_inner()), (x0, y0, cw, ch)))
}

// ------------------------------------------------------------------ locate (model-free)

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 1).map(String::from).collect()
}

const STOP: &[&str] = &["the", "a", "an", "in", "on", "of", "to", "for", "and", "button", "field", "box", "menu", "item", "icon", "at", "with", "that", "this", "is"];

/// Rank tree elements by how well their label / role / value match a description. (score, index, role, label)
pub fn rank_elements<'a>(describe: &str, els: impl Iterator<Item = &'a crate::state::Element>) -> Vec<(f64, i64, String, String)> {
    let dw: Vec<String> = words(describe).into_iter().filter(|w| !STOP.contains(&w.as_str())).collect();
    let dl = describe.to_lowercase();
    let role_words = words(describe);
    let mut out = vec![];
    for e in els {
        let lab = e.label.to_lowercase();
        if lab.is_empty() && e.value.is_none() {
            continue;
        }
        let lw = words(&format!("{} {}", e.label, e.value.clone().unwrap_or_default()));
        let hit = dw.iter().filter(|w| lw.contains(w)).count() as f64;
        let mut sc = if dw.is_empty() { 0.0 } else { hit / dw.len() as f64 };
        if !lab.is_empty() && dl.contains(&format!("\"{lab}\"")) { sc += 1.0; }
        if !lab.is_empty() && lab.len() > 2 && dl.contains(&lab) { sc += 0.5; }
        if role_words.iter().any(|w| e.role.to_lowercase().contains(w.as_str())) { sc += 0.2; }
        if sc > 0.0 {
            out.push((sc, e.index, e.role.clone(), e.label.clone()));
        }
    }
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    out.truncate(5);
    out
}

/// The element whose frame contains the point, smallest first (for snapping a grounder's / agent's pixel click).
pub fn element_at<'a>(x: f64, y: f64, els: impl Iterator<Item = &'a crate::state::Element>) -> Option<&'a crate::state::Element> {
    els.filter(|e| e.frame.as_ref().map(|f| f.w > 0.0 && f.h > 0.0 && x >= f.x && x <= f.x + f.w && y >= f.y && y <= f.y + f.h).unwrap_or(false))
        .min_by(|a, b| {
            let fa = a.frame.as_ref().unwrap();
            let fb = b.frame.as_ref().unwrap();
            (fa.w * fa.h).partial_cmp(&(fb.w * fb.h)).unwrap()
        })
}

/// The nearest labelled actionable element within `radius` px of a point that lies in no element (UiPath crop-and-
/// refine, model-free): (element, distance).
pub fn nearest_element<'a>(x: f64, y: f64, radius: f64, els: impl Iterator<Item = &'a crate::state::Element>) -> Option<(&'a crate::state::Element, f64)> {
    els.filter(|e| !e.label.is_empty() && e.frame.as_ref().map(|f| f.w > 0.0 && f.h > 0.0 && f.w * f.h < 400_000.0).unwrap_or(false))
        .map(|e| {
            let f = e.frame.as_ref().unwrap();
            let dx = if x < f.x { f.x - x } else if x > f.x + f.w { x - f.x - f.w } else { 0.0 };
            let dy = if y < f.y { f.y - y } else if y > f.y + f.h { y - f.y - f.h } else { 0.0 };
            (e, (dx * dx + dy * dy).sqrt())
        })
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
}

// ------------------------------------------------------------------ built-in skills

pub const SKILLS_MD: &str = include_str!("skills.md");

/// (name, keys, body) for every section of skills.md.
pub fn skills() -> Vec<(String, Vec<String>, String)> {
    let mut out = vec![];
    for sec in SKILLS_MD.split("\n## ").skip(1) {
        let mut lines = sec.lines();
        let name = lines.next().unwrap_or("").trim().to_string();
        let mut keys = vec![];
        let mut body = vec![];
        for l in lines {
            if let Some(k) = l.strip_prefix("keys:") {
                keys = k.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
            } else if !l.trim().is_empty() {
                body.push(l.trim());
            }
        }
        out.push((name, keys, body.join(" ")));
    }
    out
}

/// Skill sections for a task: environment always, each app whose keys occur as words in the task or a window title,
/// multi_apps when two or more apps match.
pub fn select_skills(task: &str, titles: &[String]) -> Vec<(String, String)> {
    let text = format!("{}\n{}", task, titles.join("\n")).to_lowercase();
    let all = skills();
    let mut picked: Vec<(String, String)> = vec![];
    let mut apps = 0;
    for (name, keys, body) in &all {
        if name == "environment" {
            picked.push((name.clone(), body.clone()));
            continue;
        }
        if name == "multi_apps" {
            continue;
        }
        if keys.iter().any(|k| has_word(&text, k)) {
            apps += 1;
            picked.push((name.clone(), body.clone()));
        }
    }
    if apps >= 2 {
        if let Some((n, _, b)) = all.iter().find(|x| x.0 == "multi_apps") {
            picked.push((n.clone(), b.clone()));
        }
    }
    picked
}

/// Model-free howto: the skill sentences sharing the most words with the query.
pub fn howto_from_skills(query: &str) -> String {
    let qw: Vec<String> = words(query).into_iter().filter(|w| !STOP.contains(&w.as_str()) && w.len() > 2).collect();
    let mut scored: Vec<(usize, String, String)> = vec![];
    for (name, _, body) in skills() {
        for sent in body.split(". ") {
            let sw = words(sent);
            let hit = qw.iter().filter(|w| sw.iter().any(|x| x.starts_with(w.as_str()))).count();
            if hit > 0 {
                scored.push((hit, name.clone(), sent.trim().trim_end_matches('.').to_string()));
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    if scored.is_empty() {
        return "[inspect howto] nothing in the built-in skills matches; try the app's menus (or the search role if configured).".into();
    }
    let mut out = String::from("[inspect howto] from the built-in skills (no search model configured):\n");
    for (_, n, t) in scored.iter().take(6) {
        out.push_str(&format!("- [{n}] {t}.\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Element, Frame};

    #[test]
    fn skills_parse_and_select() {
        let sk = skills();
        assert!(sk.iter().any(|s| s.0 == "libreoffice_calc") && sk.iter().any(|s| s.0 == "environment"));
        let p = select_skills("Sort the data in report.xlsx by date and save it", &[]);
        let names: Vec<&str> = p.iter().map(|x| x.0.as_str()).collect();
        assert!(names.contains(&"environment") && names.contains(&"libreoffice_calc") && !names.contains(&"multi_apps"));
        let p = select_skills("Copy the table from the Chrome page into a new Writer document", &[]);
        assert!(p.iter().any(|x| x.0 == "multi_apps"), "{p:?}");
        assert!(howto_from_skills("pivot table in calc").contains("Pivot"));
        // no task answers leak through skills (e.g. GIMP CMYK)
        assert!(!SKILLS_MD.to_lowercase().contains("cmyk"));
    }

    #[test]
    fn rank_and_snap() {
        let mut a = Element::new(3, "push button", "Bold");
        a.frame = Some(Frame { x: 100.0, y: 10.0, w: 20.0, h: 20.0 });
        let mut b = Element::new(4, "push button", "Italic");
        b.frame = Some(Frame { x: 125.0, y: 10.0, w: 20.0, h: 20.0 });
        let mut c = Element::new(5, "text", "Document");
        c.frame = Some(Frame { x: 0.0, y: 50.0, w: 800.0, h: 400.0 });
        let els = vec![a, b, c];
        let r = rank_elements("the Bold button in the toolbar", els.iter());
        assert_eq!(r[0].1, 3);
        assert_eq!(element_at(110.0, 20.0, els.iter()).unwrap().index, 3);
        assert!(element_at(122.0, 20.0, els.iter()).is_none());
        assert_eq!(nearest_element(122.0, 20.0, 12.0, els.iter()).unwrap().0.index, 3);
        assert!(nearest_element(400.0, 30.0, 12.0, els.iter()).is_none());
    }

    #[test]
    fn read_files() {
        let d = std::env::temp_dir().join(format!("gg-insp-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let t = d.join("a.txt");
        std::fs::write(&t, "hello\nworld").unwrap();
        let r = read_file_text(t.to_str().unwrap(), 100);
        assert!(r.contains("hello\nworld"));
        assert!(read_file_text(t.to_str().unwrap(), 3).contains("more characters"));
        assert!(read_file_text("/no/such/file", 10).contains("does not exist"));
        assert!(read_file_text(d.to_str().unwrap(), 100).contains("a.txt"));
        let k = d.join("rev.key");
        std::fs::write(&k, "sk-secret").unwrap();
        std::env::set_var("GUI_GATE_ROLE_TESTX_KEY_FILE", k.to_str().unwrap());
        assert!(read_file_text(k.to_str().unwrap(), 100).contains("refused") && !read_file_text(k.to_str().unwrap(), 100).contains("sk-secret"));
        std::env::remove_var("GUI_GATE_ROLE_TESTX_KEY_FILE");
        let f = env_facts("Open \"a.txt\" and the missing \"zz_nope.docx\" in LibreOffice Writer", "");
        assert!(f.contains("zz_nope.docx\": NOT found") && f.contains("hardware:"), "{f}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ws_frame_roundtrip() {
        let f = ws_frame(b"{\"id\":1}");
        assert_eq!(f[0], 0x81);
        assert_eq!((f[1] & 0x7f) as usize, 8);
        // server frames are unmasked
        let srv = [vec![0x81u8, 3], b"abc".to_vec()].concat();
        assert_eq!(ws_read(&mut &srv[..]).unwrap(), b"abc");
    }

    #[test]
    fn crop_zoom_png() {
        let img = image::RgbImage::from_fn(100, 60, |x, _| image::Rgb([x as u8, 0, 0]));
        let mut buf = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgb8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let (b, crop) = crop_zoom(&B64.encode(buf.into_inner()), 90.0, 50.0, 40.0, 40.0, 2.0).unwrap();
        assert_eq!(crop, (90, 50, 10, 10));
        let out = image::load_from_memory(&B64.decode(b).unwrap()).unwrap();
        assert_eq!((out.width(), out.height()), (20, 20));
    }
}
