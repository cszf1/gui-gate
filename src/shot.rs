//! Screenshot re-encoding (v0.9.4): every image the gate hands back to the agent is re-encoded from PNG to a
//! full-size JPEG (default quality 60). Experiment results/screenshot-format-2026-10-21: JPEG q60 at full
//! resolution answered as accurately as PNG with a cold time-to-first-token of ~7.0 s instead of ~21.4 s;
//! half-size and WebP were worse. The image is NEVER resized, so x/y coordinates the agent reads off it stay
//! valid. Anything that is not a decodable PNG (already JPEG, garbage, ...) passes through unchanged.
//!
//! GUI_GATE_SCREENSHOT_FORMAT = jpeg (default) | png      png = leave the driver's PNG untouched
//! GUI_GATE_SCREENSHOT_QUALITY = 60 (default), clamped to 30..=95
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::ImageFormat;
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub struct ShotCfg {
    pub jpeg: bool,
    pub quality: u8,
}

impl ShotCfg {
    pub fn parse(format: Option<&str>, quality: Option<&str>) -> ShotCfg {
        let jpeg = !matches!(format.map(|f| f.trim().to_lowercase()).as_deref(), Some("png"));
        let q = quality.and_then(|q| q.trim().parse::<i64>().ok()).unwrap_or(60).clamp(30, 95) as u8;
        ShotCfg { jpeg, quality: q }
    }
    pub fn from_env() -> ShotCfg {
        ShotCfg::parse(std::env::var("GUI_GATE_SCREENSHOT_FORMAT").ok().as_deref(),
                       std::env::var("GUI_GATE_SCREENSHOT_QUALITY").ok().as_deref())
    }
    pub fn name(&self) -> String {
        if self.jpeg { format!("jpeg q{}", self.quality) } else { "png".into() }
    }
}

/// PNG bytes -> JPEG bytes at the same width/height. None when the input is not a decodable PNG.
pub fn png_to_jpeg(png: &[u8], quality: u8) -> Option<Vec<u8>> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let img = image::load_from_memory_with_format(png, ImageFormat::Png).ok()?;
    let rgb = img.to_rgb8(); // JPEG has no alpha; screenshots are opaque
    let mut out = Vec::with_capacity(png.len() / 3);
    JpegEncoder::new_with_quality(&mut out, quality).encode_image(&rgb).ok()?;
    Some(out)
}

/// Re-encode the image parts of an MCP tool result in place. Returns a log record when there were images.
pub fn reencode_result(result: &mut Value, cfg: &ShotCfg) -> Option<Value> {
    let arr = result.get_mut("content")?.as_array_mut()?;
    let t0 = Instant::now();
    let (mut n, mut converted, mut bin, mut bout) = (0u64, 0u64, 0u64, 0u64);
    for c in arr.iter_mut() {
        if c.get("type").and_then(|t| t.as_str()) != Some("image") {
            continue;
        }
        n += 1;
        let data = c.get("data").and_then(|d| d.as_str()).unwrap_or("");
        let raw = B64.decode(data.as_bytes()).ok();
        let in_len = raw.as_ref().map(|r| r.len()).unwrap_or(data.len() * 3 / 4) as u64;
        bin += in_len;
        let mime = c.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
        let jpg = if cfg.jpeg && (mime.is_empty() || mime == "image/png") { raw.as_deref().and_then(|r| png_to_jpeg(r, cfg.quality)) } else { None };
        match jpg {
            Some(j) => {
                bout += j.len() as u64;
                converted += 1;
                c["data"] = json!(B64.encode(&j));
                c["mimeType"] = json!("image/jpeg");
            }
            None => bout += in_len, // not a PNG / decoding failed / png setting: original untouched
        }
    }
    if n == 0 {
        return None;
    }
    Some(json!({"ev": "img", "format": cfg.name(), "images": n, "converted": converted, "bytes_in": bin, "bytes_out": bout,
                "ms": (t0.elapsed().as_secs_f64() * 10000.0).round() / 10.0}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screenshot-like PNG: flat UI panels, text-ish stripes and a gradient, RGBA like the driver's captures.
    fn fake_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| {
            let panel = if y < 40 { 230 } else if x < 220 { 245 } else { 255 };
            let stripe = if (y / 6) % 4 == 0 && (x / 3) % 5 != 0 && x % 300 < 180 { 40 } else { panel };
            image::Rgba([stripe as u8, (stripe as u32 * (w - x / 2) / w) as u8, ((x + y) % 256) as u8, 255])
        });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn cfg_parsing_and_clamp() {
        assert_eq!(ShotCfg::parse(None, None), ShotCfg { jpeg: true, quality: 60 });
        assert_eq!(ShotCfg::parse(Some("PNG"), Some("80")), ShotCfg { jpeg: false, quality: 80 });
        assert_eq!(ShotCfg::parse(Some("jpeg"), Some("5")).quality, 30);
        assert_eq!(ShotCfg::parse(Some("jpeg"), Some("100")).quality, 95);
        assert_eq!(ShotCfg::parse(Some("jpeg"), Some("x")).quality, 60);
    }

    #[test]
    fn png_becomes_smaller_valid_jpeg_same_size() {
        let png = fake_png(1396, 823);
        let t0 = Instant::now();
        let jpg = png_to_jpeg(&png, 60).expect("encodes");
        eprintln!("re-encode 1396x823: {:.1} ms, {} -> {} bytes", t0.elapsed().as_secs_f64() * 1000.0, png.len(), jpg.len());
        assert!(jpg.len() < png.len(), "{} !< {}", jpg.len(), png.len());
        assert!(jpg.starts_with(&[0xFF, 0xD8, 0xFF]));
        let back = image::load_from_memory_with_format(&jpg, ImageFormat::Jpeg).expect("valid jpeg");
        assert_eq!((back.width(), back.height()), (1396, 823));
    }

    #[test]
    fn result_rewritten_and_bad_data_passed_through() {
        let png = fake_png(200, 120);
        let mut r = json!({"content": [{"type": "text", "text": "t"},
                                       {"type": "image", "data": B64.encode(&png), "mimeType": "image/png"},
                                       {"type": "image", "data": "AAAA", "mimeType": "image/png"}]});
        let ev = reencode_result(&mut r, &ShotCfg { jpeg: true, quality: 60 }).unwrap();
        assert_eq!(r["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(r["content"][2], json!({"type": "image", "data": "AAAA", "mimeType": "image/png"}));
        assert_eq!(ev["images"], 2);
        assert_eq!(ev["converted"], 1);
        assert!(ev["bytes_out"].as_u64().unwrap() < ev["bytes_in"].as_u64().unwrap());
        // png setting: untouched
        let mut r2 = json!({"content": [{"type": "image", "data": B64.encode(&png), "mimeType": "image/png"}]});
        let before = r2.clone();
        reencode_result(&mut r2, &ShotCfg { jpeg: false, quality: 60 }).unwrap();
        assert_eq!(r2, before);
        // no images: no log record
        assert!(reencode_result(&mut json!({"content": [{"type": "text", "text": "x"}]}), &ShotCfg::parse(None, None)).is_none());
    }

    /// Timing on a real-size screenshot in an optimised build: `cargo test --release shot::tests::timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        // GG_TIMING_DIR = a folder of real screenshot PNGs; otherwise a synthetic 1396x823 one
        let mut pngs: Vec<(String, Vec<u8>)> = std::env::var("GG_TIMING_DIR").ok()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| rd.flatten().filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
                 .map(|e| (e.file_name().to_string_lossy().to_string(), std::fs::read(e.path()).unwrap())).collect())
            .unwrap_or_default();
        if pngs.is_empty() {
            pngs.push(("synthetic".into(), fake_png(1396, 823)));
        }
        pngs.sort();
        let reps = 5;
        let (mut total, mut worst, mut bin, mut bout) = (0.0f64, 0.0f64, 0usize, 0usize);
        for (name, png) in &pngs {
            let b64 = B64.encode(png);
            let mut best = f64::MAX;
            let mut jl = 0;
            for _ in 0..reps {
                let mut r = json!({"content": [{"type": "image", "data": b64.clone(), "mimeType": "image/png"}]});
                let t0 = Instant::now();
                reencode_result(&mut r, &ShotCfg::parse(None, None)).unwrap();
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                best = best.min(ms);
                total += ms;
                worst = worst.max(ms);
                jl = B64.decode(r["content"][0]["data"].as_str().unwrap()).unwrap().len();
            }
            bin += png.len();
            bout += jl;
            eprintln!("TIMING {name}: best {best:.1} ms, png {} B -> jpeg {} B", png.len(), jl);
        }
        eprintln!("TIMING summary: {} screenshots x {reps}: mean {:.1} ms, worst {:.1} ms; bytes {bin} -> {bout} ({:.1}%)",
                  pngs.len(), total / (pngs.len() * reps) as f64, worst, 100.0 * bout as f64 / bin as f64);
    }
}
