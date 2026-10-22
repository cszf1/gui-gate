//! Loop breaker: when the agent's recent actions keep making no progress (driver errors, no effect, or steps it
//! already tried a moment ago), the gate says so in the result and names a different *mode* of acting; at level 2
//! it refuses re-sending one of those no-progress steps (nothing executed), at level 3 it adds the last exits
//! (run_command, ask the person, declare the task infeasible). Rules only; the log replay uses the same code.
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Entry {
    pub sig: String,
    pub mode: String,
    pub np: bool,
    pub confirmed: bool,
}

#[derive(Default, Clone, Debug)]
pub struct LoopTrack {
    pub hist: VecDeque<Entry>,
    pub level: u8,
    pub progress_run: u32,
    /// 0.9.7 GUI_GATE_TWO_STRIKES: mechanism of each recent step and the window state after it
    pub mechs: VecDeque<(String, bool)>,
    pub states: VecDeque<String>,
    pub last_reason: &'static str,
}

/// The way an action acts on the UI.
pub fn mode_of(tool: &str, kind: &str) -> &'static str {
    match tool {
        "press_key" | "hotkey" => "keyboard",
        "type_text" | "set_value" => "typing",
        "invoke_menu" => "menu",
        "run_command" => "command",
        "scroll" => "scroll",
        _ if kind == "pixel" => "pixel",
        _ => "click",
    }
}

pub fn enabled() -> bool {
    std::env::var("GUI_GATE_LOOP_BREAKER").map(|v| v != "0").unwrap_or(true)
}

const WINDOWS: [(usize, usize); 3] = [(6, 4), (8, 6), (12, 9)]; // (last N actions, no-progress among them) per level

impl LoopTrack {
    /// True when `sig` was sent among the last `n` actions without a confirmed effect (a step that worked and is
    /// repeated on purpose - the same menu item for the next paragraph - is not a retry).
    pub fn seen_recent(&self, sig: &str, n: usize) -> bool {
        self.hist.iter().rev().take(n).any(|e| e.sig == sig && !e.confirmed)
    }

    /// Record an action. `failed`: driver error, no effect, or sent back by the gate; `confirmed`: the driver
    /// confirmed its effect. Returns the new level when it rose.
    pub fn push(&mut self, sig: &str, mode: &str, failed: bool, confirmed: bool) -> Option<u8> {
        let np = failed || (!confirmed && self.seen_recent(sig, 6));
        self.hist.push_back(Entry { sig: sig.into(), mode: mode.into(), np, confirmed: confirmed && !failed });
        while self.hist.len() > 12 {
            self.hist.pop_front();
        }
        if np {
            self.progress_run = 0;
        } else {
            self.progress_run += 1;
            if self.progress_run >= 3 {
                // three new, working steps in a row: the loop is broken
                self.hist.clear();
                self.level = 0;
                return None;
            }
        }
        let mut lvl = 0;
        for (i, (n, k)) in WINDOWS.iter().enumerate() {
            if self.hist.len() >= *n && self.hist.iter().rev().take(*n).filter(|e| e.np).count() >= *k {
                lvl = i as u8 + 1;
            }
        }
        if lvl > self.level {
            self.level = lvl;
            return Some(lvl);
        }
        None
    }

    /// Signatures of the no-progress steps in the window.
    pub fn failing_sigs(&self) -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for e in self.hist.iter().filter(|e| e.np) {
            if !v.contains(&e.sig) {
                v.push(e.sig.clone());
            }
        }
        v
    }

    /// The mode most of the no-progress steps used.
    pub fn dominant_mode(&self) -> String {
        let mut best = ("", 0usize);
        for e in self.hist.iter().filter(|e| e.np) {
            let c = self.hist.iter().filter(|x| x.np && x.mode == e.mode).count();
            if c > best.1 {
                best = (e.mode.as_str(), c);
            }
        }
        best.0.to_string()
    }

    /// Level >= 2: re-sending one of the no-progress steps is refused.
    pub fn blocks(&self, sig: &str) -> bool {
        self.level >= 2 && self.hist.iter().any(|e| e.np && e.sig == sig) && !self.hist.iter().any(|e| e.confirmed && e.sig == sig)
    }
}

impl LoopTrack {
    /// 0.9.7 (Pointer "two strikes, then switch"; VLAA tier 1 modality switch / tier 2 recurring screen state;
    /// OS-Symphony screen-similarity loop detection). On top of `push`: the same mechanism (mode + target) failing twice
    /// in a row raises level 1 at once; the window state after an action recurring 3 times within the last 8 actions
    /// (with no progress in between) raises level 2. `state`: a fingerprint of the window after the action.
    pub fn push_ext(&mut self, sig: &str, mode: &str, mech: &str, failed: bool, confirmed: bool, state: Option<String>) -> Option<u8> {
        let before = self.level;
        let base = self.push(sig, mode, failed, confirmed);
        let np = self.hist.back().map(|e| e.np).unwrap_or(false);
        if self.hist.is_empty() {
            // push() just reset the tracker after three working steps
            self.mechs.clear();
            self.states.clear();
            self.last_reason = "";
            return None;
        }
        self.mechs.push_back((mech.to_string(), np));
        while self.mechs.len() > 4 {
            self.mechs.pop_front();
        }
        if let Some(st) = state {
            self.states.push_back(if np { st } else { format!("{st}#{}", self.hist.len()) });
            while self.states.len() > 8 {
                self.states.pop_front();
            }
        }
        let mut lvl = self.level;
        let mut reason = if base.is_some() { "window" } else { "" };
        let n = self.mechs.len();
        if n >= 2 && self.mechs[n - 1].1 && self.mechs[n - 2].1 && self.mechs[n - 1].0 == self.mechs[n - 2].0 && lvl < 1 {
            lvl = 1;
            reason = "two_strikes";
        }
        if let Some(last) = self.states.back() {
            if !last.contains('#') && self.states.iter().filter(|x| *x == last).count() >= 3 && lvl < 2 {
                lvl = 2;
                reason = "state_repeat";
            }
        }
        if lvl > before {
            self.level = lvl;
            self.last_reason = reason;
            return Some(lvl);
        }
        base
    }
}

/// The note for a level reached by push_ext.
pub fn note_ext(level: u8, t: &LoopTrack) -> String {
    match t.last_reason {
        "two_strikes" => format!("[gate] Two strikes: the same mechanism ({}) failed twice in a row. Do not try a third variation of it - switch to a genuinely different mechanism now: {}.",
                                 t.mechs.back().map(|m| m.0.replace('|', " on ")).unwrap_or_default(), switch_advice(&t.dominant_mode())),
        "state_repeat" => format!("[gate] Loop breaker (level {level}/3): the window came back to the same state 3 times after your actions. Change the overall strategy (menu navigation -> keyboard shortcuts / a different dialog / code with run_command / ask the person), not just the control. Re-sending one of the steps that led back here is now refused."),
        _ => note(level, t),
    }
}

pub fn switch_advice(mode: &str) -> &'static str {
    match mode {
        "click" | "pixel" => "stop clicking that control: use the keyboard (the app's shortcut, F10 or Alt+letter to open menus and arrows/Enter to pick, Tab to move between fields), invoke_menu with the menu path, or the same element_token with delivery_mode \"background\"",
        "keyboard" => "stop sending those keys: click the control itself by element_token from the tree, or use invoke_menu with the menu path",
        "typing" => "stop re-typing: click the field first (element_token), select its content (ctrl+a), or use set_value on the element",
        "menu" => "stop using that menu path: use the app's keyboard shortcut or click the control by element_token",
        "command" => "do not re-run the same command: read its error, change the command, or do this step in the app",
        "scroll" => "stop scrolling: move with the keyboard (ctrl+Home/End, Page Down) or jump directly (e.g. the Name Box in Calc, ctrl+g / ctrl+l)",
        _ => "use a clearly different way: keyboard shortcut, a click on another element, invoke_menu, or run_command",
    }
}

pub fn note(level: u8, t: &LoopTrack) -> String {
    let n = t.hist.iter().rev().take(WINDOWS[(level as usize).saturating_sub(1).min(2)].0).filter(|e| e.np).count();
    let mode = t.dominant_mode();
    let tried: Vec<String> = t.failing_sigs().iter().rev().take(3).map(|s| s.split('|').filter(|x| !x.is_empty() && *x != "None" && x.parse::<i64>().is_err()).collect::<Vec<_>>().join(" ")).collect();
    let mut s = format!("[gate] Loop breaker (level {level}/3): {n} of your recent actions made no progress (errors, no effect, or steps you had already tried: {}). Switch mode now - {}.",
                        tried.join("; "), switch_advice(&mode));
    if level >= 2 {
        s.push_str(" Re-sending one of those steps is now refused.");
    }
    if level >= 3 {
        s.push_str(" If no other mode works: do the work in code with run_command (files, documents, settings files), ask the person with request_human, or, if the task cannot be done, reply FAIL (set_plan with feasible:false).");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_rise_and_reset() {
        let mut t = LoopTrack::default();
        assert_eq!(t.push("click|1|button|Save", "click", true, false), None);
        assert_eq!(t.push("click|1|button|Save", "click", true, false), None);
        assert_eq!(t.push("press_key|1|f2", "keyboard", false, false), None);
        assert_eq!(t.push("click|1|button|Save", "click", false, false), None); // seen again = no progress
        assert_eq!(t.push("press_key|1|f2", "keyboard", false, false), None);
        assert_eq!(t.push("click|1|button|Save", "click", true, false), Some(1)); // 5 of 6
        assert!(!t.blocks("click|1|button|Save"));
        t.push("press_key|1|f2", "keyboard", false, false);
        assert_eq!(t.push("click|1|button|Save", "click", true, false), Some(2));
        assert!(t.blocks("click|1|button|Save") && !t.blocks("click|1|button|Other"));
        assert_eq!(t.dominant_mode(), "click");
        let n = note(2, &t);
        assert!(n.contains("level 2/3") && n.contains("keyboard") && n.contains("refused"), "{n}");
        for s in ["a", "b", "c", "d"] {
            t.push(&format!("click|1|button|{s}"), "click", false, false);
        }
        assert_eq!(t.level, 0);
        assert!(!t.blocks("click|1|button|Save"));
    }

    #[test]
    fn working_varied_steps_never_fire() {
        let mut t = LoopTrack::default();
        for i in 0..30 {
            assert_eq!(t.push(&format!("click|1|button|b{i}"), "click", false, false), None);
        }
    }

    #[test]
    fn a_confirmed_step_repeated_on_purpose_is_progress() {
        let mut t = LoopTrack::default();
        for _ in 0..10 {
            assert_eq!(t.push("click|1|check menu item|Line Spacing: 2", "click", false, true), None);
            assert_eq!(t.push("press_key|1|o", "keyboard", false, true), None);
        }
        assert!(!t.blocks("click|1|check menu item|Line Spacing: 2"));
    }

    #[test]
    fn level_three_mentions_the_exits() {
        let mut t = LoopTrack::default();
        let mut top = 0;
        for _ in 0..12 {
            if let Some(l) = t.push("click|1|cell|A1", "click", true, false) {
                top = l;
            }
        }
        assert_eq!(top, 3);
        assert!(note(3, &t).contains("run_command") && note(3, &t).contains("FAIL"));
    }

    #[test]
    fn two_strikes_and_recurring_states() {
        let mut t = LoopTrack::default();
        assert_eq!(t.push_ext("click|1|button|Save|a", "click", "click|Save", true, false, None), None);
        assert_eq!(t.push_ext("click|1|button|Save|b", "click", "click|Save", true, false, None), Some(1));
        assert!(note_ext(1, &t).starts_with("[gate] Two strikes") && note_ext(1, &t).contains("click on Save"));
        // a different mechanism failing is not a second strike of the first
        let mut t = LoopTrack::default();
        t.push_ext("click|1|button|Save", "click", "click|Save", true, false, None);
        assert_eq!(t.push_ext("hotkey|1|ctrl+s", "keyboard", "keyboard|win", true, false, None), None);
        // the same screen state three times with no progress -> level 2
        let mut t = LoopTrack::default();
        let mut got = None;
        for (i, m) in ["click|A", "keyboard|w", "click|B", "keyboard|x", "click|C"].iter().enumerate() {
            let r = t.push_ext(&format!("s{i}"), m.split('|').next().unwrap(), m, i % 2 == 0, false, Some("Dialog|abc".into()));
            if r.is_some() { got = r; }
        }
        assert_eq!(got, Some(2));
        assert_eq!(t.last_reason, "state_repeat");
        // working steps on the same state do not count
        let mut t = LoopTrack::default();
        for i in 0..6 {
            assert_eq!(t.push_ext(&format!("ok{i}"), "click", &format!("click|b{i}"), false, true, Some("Same|x".into())), None);
        }
    }

    /// Offline replay: GG_LOOP_LOGS=<file with one gate log path per line> GG_LOOP_OUT=<tsv>
    /// cargo test -- --ignored loop_replay. One row per would-be event: log, aid, kind (note/block), level.
    #[test]
    #[ignore]
    fn loop_replay() {
        let (Ok(inp), Ok(outp)) = (std::env::var("GG_LOOP_LOGS"), std::env::var("GG_LOOP_OUT")) else { return };
        let mut out = String::new();
        for path in std::fs::read_to_string(inp).unwrap().lines() {
            let Ok(txt) = std::fs::read_to_string(path) else { continue };
            let mut t = LoopTrack::default();
            let mut n_act = 0;
            for line in txt.lines() {
                let Ok(e) = serde_json::from_str::<serde_json::Value>(line) else { continue };
                if e["ev"] != "action" {
                    continue;
                }
                let dec = e["decision"].as_str().unwrap_or("");
                let refused = e["executed"] != true && ["look_light", "look_full", "stop_loop", "ask_help", "review"].contains(&dec);
                if e["executed"] != true && !refused {
                    continue;
                }
                n_act += 1;
                let tool = e["tool"].as_str().unwrap_or("");
                let sig = e["sig"].as_str().unwrap_or("");
                let kind = e["target"]["kind"].as_str().unwrap_or("");
                let keys = sig.rsplit('|').next().unwrap_or("").to_lowercase();
                if (tool == "press_key" || tool == "hotkey") && ["up", "down", "left", "right", "tab", "page_down", "page_up", "pagedown", "pageup"].contains(&keys.as_str()) {
                    continue;
                }
                let aid = e["aid"].as_i64().unwrap_or(0);
                if t.blocks(sig) {
                    out.push_str(&format!("{path}\t{aid}\tblock\t{}\t{}\n", t.level, mode_of(tool, kind)));
                }
                let failed = refused || e["is_error"] == true || e["effect"] == "none";
                if let Some(l) = t.push(sig, mode_of(tool, kind), failed, e["effect"] == "confirmed") {
                    out.push_str(&format!("{path}\t{aid}\tnote\t{l}\t{}\n", t.dominant_mode()));
                }
            }
            out.push_str(&format!("{path}\t{n_act}\tactions\t0\t-\n"));
        }
        std::fs::write(outp, out).unwrap();
    }
}
