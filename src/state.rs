//! What the gate knows at runtime: every observation and action that passed through it.
//! Nothing here uses ground truth that an agent could not have.
use crate::util::*;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

fn row_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\[(\d+)\]\s+([^"\[]+?)\s+"((?:[^"\\]|\\.)*)""#).unwrap())
}
fn row_nolabel_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[(\d+)\]\s+([a-z][a-z \-]*?)(?:\s+\[|\s*$)").unwrap())
}
pub fn loading_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)(progress bar|busy|loading|spinner|加载|正在|请稍候|please wait)").unwrap())
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Element {
    pub index: i64,
    pub role: String,
    pub label: String,
    pub frame: Option<Frame>,
    pub enabled: Option<bool>,
}

impl Element {
    pub fn new(index: i64, role: &str, label: &str) -> Self {
        Element { index, role: role.into(), label: label.into(), frame: None, enabled: None }
    }
}

pub type ObsId = usize;

#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub snapshot_id: String,
    pub pid: Option<i64>,
    pub window_id: Option<i64>,
    pub t: f64,
    pub with_screenshot: bool,
    pub elements: IndexMap<i64, Element>,
    pub shot_w: Option<f64>,
    pub shot_h: Option<f64>,
    pub loading_cue: bool,
    pub truncated: bool,
    pub window_count: Option<String>, // window signature of this pid at read time
    pub text: String,                 // the read's text (tree incl. unindexed labels such as a dialog's message)
    pub gate_read: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ActionRecord {
    pub t: f64,
    pub tool: String,
    pub key: String,
    pub window_key: Option<String>,
    pub is_error: bool,
    pub unverifiable: bool,
    pub popup: bool,
    pub stale_error: bool,
    pub aid: u64,
}

fn frame_of(v: Option<&Value>) -> Option<Frame> {
    let f = v?.as_object()?;
    let g = |k: &str| f.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some(Frame { x: g("x"), y: g("y"), w: g("w"), h: g("h") })
}

pub fn parse_elements(sc: &Value) -> IndexMap<i64, Element> {
    let mut els: IndexMap<i64, Element> = IndexMap::new();
    if let Some(arr) = sc.get("elements").and_then(|e| e.as_array()) {
        for e in arr {
            let Some(idx) = as_i64(e.get("element_index")) else { continue };
            els.insert(idx, Element {
                index: idx,
                role: pystr(e.get("role")),
                label: pystr(e.get("label")),
                frame: frame_of(e.get("frame")),
                enabled: e.get("enabled").and_then(|b| b.as_bool()),
            });
        }
    }
    let md = s(sc, "tree_markdown");
    for line in md.lines() {
        if let Some(m) = row_re().captures(line) {
            let idx: i64 = m[1].parse().unwrap_or(-1);
            els.entry(idx).or_insert_with(|| Element::new(idx, m[2].trim(), &m[3]));
            continue;
        }
        if let Some(m) = row_nolabel_re().captures(line) {
            let idx: i64 = m[1].parse().unwrap_or(-1);
            els.entry(idx).or_insert_with(|| Element::new(idx, m[2].trim(), ""));
        }
    }
    els
}

#[derive(Default)]
pub struct GateState {
    pub obs: Vec<Observation>,
    pub obs_by_snapshot: HashMap<String, ObsId>,
    pub latest_by_window: IndexMap<String, ObsId>,
    pub actions: Vec<ActionRecord>,
    pub window_of_pid: HashMap<i64, i64>,
    pub last_typed: HashMap<i64, String>, // pid -> text typed since the last Enter
    shots: u64,
}

pub fn wkey(pid: Option<i64>, wid: Option<i64>) -> Option<String> {
    if pid.is_none() && wid.is_none() {
        return None;
    }
    let f = |x: Option<i64>| x.map(|v| v.to_string()).unwrap_or_else(|| "None".into());
    Some(format!("{}:{}", f(pid), f(wid)))
}

impl GateState {
    pub fn get(&self, id: ObsId) -> &Observation {
        &self.obs[id]
    }
    pub fn get_mut(&mut self, id: ObsId) -> &mut Observation {
        &mut self.obs[id]
    }

    fn push(&mut self, o: Observation) -> ObsId {
        self.obs.push(o);
        self.obs.len() - 1
    }

    pub fn record_observation(&mut self, args: &Value, result: &Value, now_t: Option<f64>) -> Option<ObsId> {
        let sc = sc(result);
        let snap = sc.get("snapshot_id").and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from);
        let pid = as_i64(sc.get("pid")).or_else(|| as_i64(args.get("pid")));
        let wid = if sc.get("window_id").is_some() { as_i64(sc.get("window_id")) } else { as_i64(args.get("window_id")) };
        let with_shot = result.get("content").and_then(|c| c.as_array()).map(|a| a.iter().any(|c| s(c, "type") == "image"))
            .unwrap_or(false) || truthy(sc.get("capture_id"));
        let key = wkey(pid, wid).unwrap_or_default();
        let t = now_t.unwrap_or_else(now);
        let Some(snap) = snap else {
            if !with_shot || pid.is_none() {
                return None;
            }
            // a screenshot-only read (no tree): still a fresh look at the window for pixel actions
            let prev = self.latest_by_window.get(&key).map(|&i| self.obs[i].clone());
            self.shots += 1;
            let o = Observation {
                snapshot_id: prev.as_ref().map(|p| p.snapshot_id.clone()).unwrap_or_else(|| format!("shot{}", self.shots)),
                pid, window_id: wid, t, with_screenshot: true,
                elements: prev.as_ref().map(|p| p.elements.clone()).unwrap_or_default(),
                shot_w: as_f64(sc.get("screenshot_width")), shot_h: as_f64(sc.get("screenshot_height")),
                text: prev.as_ref().map(|p| p.text.clone()).unwrap_or_default(),
                ..Default::default()
            };
            let snapid = o.snapshot_id.clone();
            let id = self.push(o);
            self.latest_by_window.insert(key, id);
            if prev.is_none() {
                self.obs_by_snapshot.insert(snapid, id);
            }
            if let (Some(p), Some(w)) = (pid, wid) {
                self.window_of_pid.insert(p, w);
            }
            return Some(id);
        };
        let mut els = parse_elements(&sc);
        let prev = self.latest_by_window.get(&key).map(|&i| i);
        let md = s(&sc, "tree_markdown").to_string();
        let mut alltext = md.clone() + "\n";
        alltext.push_str(&text_of(result));
        let since = truthy(args.get("since"));
        let no_change = alltext.contains("no change since");
        let diff_read = since && prev.is_some() && !alltext.contains("full read follows") && !no_change;
        if since && no_change {
            if let Some(p) = prev {
                els = self.obs[p].elements.clone();
            }
        } else if diff_read {
            let mut merged = self.obs[prev.unwrap()].elements.clone();
            for (k, v) in els {
                merged.insert(k, v);
            }
            els = merged;
        }
        let loading = loading_re().is_match(&md) || els.values().any(|e| loading_re().is_match(&format!("{} {}", e.role, e.label)));
        let text = match prev {
            Some(p) if no_change => self.obs[p].text.clone(),
            _ => trunc(&alltext, 12000),
        };
        let o = Observation {
            snapshot_id: snap.clone(), pid, window_id: wid, t, with_screenshot: with_shot, elements: els,
            shot_w: as_f64(sc.get("screenshot_width")), shot_h: as_f64(sc.get("screenshot_height")),
            loading_cue: loading, truncated: truthy(sc.get("truncated")), text, ..Default::default()
        };
        let id = self.push(o);
        self.obs_by_snapshot.insert(snap, id);
        self.latest_by_window.insert(key, id);
        if let (Some(p), Some(w)) = (pid, wid) {
            self.window_of_pid.insert(p, w);
        }
        Some(id)
    }

    pub fn latest_for(&self, pid: Option<i64>, window_id: Option<i64>) -> Option<ObsId> {
        let wid = if window_id.is_none() { pid.and_then(|p| self.window_of_pid.get(&p).copied()) } else { window_id };
        if let Some(k) = wkey(pid, wid) {
            if let Some(&i) = self.latest_by_window.get(&k) {
                return Some(i);
            }
        }
        if let Some(p) = pid {
            let pre = format!("{p}:");
            for (k, &v) in &self.latest_by_window {
                if k.starts_with(&pre) {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn record_action(&mut self, rec: ActionRecord) {
        self.actions.push(rec);
        if self.actions.len() > 500 {
            let cut = self.actions.len() - 500;
            self.actions.drain(..cut);
        }
    }

    pub fn actions_since(&self, t: f64, window_key: Option<&str>) -> usize {
        self.actions.iter().filter(|a| a.t > t && (window_key.is_none() || a.window_key.is_none()
            || a.window_key.as_deref() == window_key)).count()
    }

    pub fn last_action(&self) -> Option<&ActionRecord> {
        self.actions.last()
    }

    /// Consecutive identical actions with no observation in between.
    pub fn repeat_count(&self, key: &str, since: f64) -> usize {
        let mut n = 0;
        for a in self.actions.iter().rev() {
            if a.t <= since {
                break;
            }
            if a.key == key {
                n += 1;
            } else {
                break;
            }
        }
        n
    }

    /// Observations of any window of this pid newer than `cut`, newest last-read.
    pub fn newest_for_pid(&self, pid: i64, cut: f64) -> Option<ObsId> {
        let pre = format!("{pid}:");
        self.latest_by_window.iter().filter(|(k, &i)| k.starts_with(&pre) && self.obs[i].t >= cut)
            .max_by(|a, b| self.obs[*a.1].t.partial_cmp(&self.obs[*b.1].t).unwrap())
            .map(|(_, &i)| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn screenshot_only_read_counts_as_observation() {
        let mut st = GateState::default();
        let res = json!({"structuredContent": {"pid": 77, "screenshot_width": 800, "screenshot_height": 600},
                         "content": [{"type": "image", "data": "x", "mimeType": "image/png"}]});
        let o = st.record_observation(&json!({"pid": 77, "include_screenshot": true}), &res, None).unwrap();
        assert!(st.get(o).with_screenshot);
        assert_eq!(st.latest_for(Some(77), Some(5)), Some(o));
    }

    #[test]
    fn rows_parse() {
        let md = "- frame = \"F\"\n    - [0] push button \"Save\" [actions=[click]]\n    - [3] separator\n";
        let els = parse_elements(&json!({"tree_markdown": md}));
        assert_eq!(els[&0].role, "push button");
        assert_eq!(els[&0].label, "Save");
        assert_eq!(els[&3].role, "separator");
    }
}
