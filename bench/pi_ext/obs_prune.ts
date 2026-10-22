// Pi extension (opt-in: PI_OBS_PRUNE=1 in bench/run_bench.sh): context hygiene for GUI runs.
// Before each LLM call, keep only the newest window observation in full; older ones (get_window_state results and
// the "[gate] Window right after the action" part of action results) are replaced by a one-line summary and lose
// their screenshots. The session file keeps everything (the `context` event is non-destructive).
// Same rule as bench/context_hygiene.py (prune()), which the offline replay and the parity test use.
// Plain JavaScript syntax on purpose (valid TypeScript; the parity test imports it with node).
const MARKERS = ["[gate] Window right after the action", "[gate] Window now (nothing was executed)"];
const KEEP_FULL = 1;
const HEAD_KEEP = 600;

function textOf(m) {
  return (m.content || []).filter((c) => c && c.type === "text").map((c) => c.text || "").join("\n");
}

export function isObs(m) {
  if (!m || m.role !== "toolResult") return false;
  const n = String(m.toolName || "");
  if (n.endsWith("get_window_state")) return true;
  const t = textOf(m);
  return MARKERS.some((k) => t.includes(k));
}

export function summary(text) {
  const title = (text.match(/(?:frame|dialog|window) = "([^"]{0,80})/) || [])[1];
  const snap = (text.match(/snapshot_id=(\S+)/) || text.match(/\(snapshot (s[0-9a-f]+)/) || [])[1];
  const rows = (text.match(/^\s*- \[\d+\]/gm) || []).length;
  return `[client: older window read summarised - "${title || "?"}", snapshot ${snap || "?"}, ${rows} rows; the newest read is current]`;
}

function pruneText(text, name) {
  for (const k of MARKERS) {
    const i = text.indexOf(k);
    if (i >= 0) {
      const head = text.slice(0, i).trimEnd();
      return (head.length > HEAD_KEEP ? head.slice(0, HEAD_KEEP) + " ..." : head) + (head ? "\n" : "") + summary(text.slice(i));
    }
  }
  if (String(name || "").endsWith("get_window_state")) return summary(text);
  return text;
}

export function prune(messages) {
  const idx = [];
  messages.forEach((m, i) => { if (isObs(m)) idx.push(i); });
  const old = new Set(idx.slice(0, Math.max(0, idx.length - KEEP_FULL)));
  return messages.map((m, i) => {
    if (!old.has(i)) return m;
    const content = [];
    for (const c of m.content || []) {
      if (!c || c.type === "image") continue;
      if (c.type === "text") content.push({ ...c, text: pruneText(c.text || "", m.toolName) });
      else content.push(c);
    }
    return { ...m, content };
  });
}

export default function (pi) {
  pi.on("context", async (event) => ({ messages: prune(event.messages) }));
}
