# screenshot-format-2026-10-21 — which screenshot format to send to the model

**Recommendation: JPEG quality 60, full size.** Its accuracy matched PNG (paired 18 better / 17 worse, sign test p = 1.0).
Time to first token was about 3x lower with 1 image (cold 7.0 s vs 21.4 s) and about 3.8x lower with 3 images (17.2 s vs 64.8 s).
The image is 93 KB instead of 326 KB, with the same prompt tokens (773 per image).
Half-size JPEG is fastest but accuracy drops sharply (25% vs 43%, paired 7 / 47, p < 1e-7), so it is rejected.
WebP q80 is faster than JPEG q60 (cold 4.9 s / 11.4 s), but its accuracy trends lower (38% vs 43%, paired 11 / 21, p = 0.11).
That is not significant, but it does not meet "no meaningful drop" as clearly as JPEG q60. WebP is a second choice to re-check with more samples.
No gate code was changed. This is a measurement only (the user implements the chosen format).

## Setup
- Model: Pi's main model **tokenrhythm deepseek-v4.1-flash-1017**, `reasoning_effort: medium` (= Pi think medium), OpenAI chat
  completions, streaming, via https://tokenrhythm.studio/v1. Box gui-pi, 2026-10-21 about 21:00-21:55 CST. The main benchmark was NOT running
  (the pi-ds41 run had been stopped at 20:51), so no interference from it.
- Screenshots: **the gate's own get_window_state screenshot** (exactly what Pi receives: window-local PNG, 1396x823 for
  LibreOffice, frame_scale 0.969). Taken right after `osworld_native.py setup` of 15 OSWorld tasks:
  Calc ow01 ow05 ow08 ow12 ow16, Writer ow02 ow07 ow11 ow14, Impress ow03 ow09 ow13, Chromium ow04, GIMP ow17 ow21
  (ow06 Chrome had no window and VLC ow10 was a 600x420 window, so both were excluded). The other formats are re-encodings of that PNG with PIL:
  `jpg80`, `jpg60` (full size), `jpg80half` (698x411, LANCZOS), `webp80`. `png` = the gate's original bytes.
- Ground truth: AT-SPI walk taken at the same moment (`groundtruth/*.full.atspi.json`) mapped to screenshot pixels
  `((x - win.x) * scale, (y + off - win.y) * scale)`. **LibreOffice reports mixed frames.** Menu-bar menus, the first-run
  infobar and the sidebar are in desktop coordinates. Toolbars, cells, the status bar and sheet tabs are relative to the decorated toplevel (+27 px).
  Targets were therefore limited to frames verified by drawing boxes on the shots: LO toolbar buttons and menus, status-bar labels, integer cells.
  Chromium and GIMP are in desktop coordinates. GIMP elements hidden under a dialog were dropped.
  73 questions: 44 click (name a UI element, answer x,y; hit = inside its box ±2 px, half-size answers scaled x2),
  29 text (read the text inside a given rectangle; exact match after whitespace collapse and trailing ':').
- Calls: every question x 5 formats x 2 repeats with 1 image (730), plus every third question x 5 x 2 with 3 images in
  context (2 other shots + the target, same format; 250). 980 calls total, shuffled so all formats share the same load, at concurrency
  5-12. The endpoint limits to 30 requests/min. 171 429/503 attempts were retried, and each attempt is timed on its own.
- "Cold" = calls where the provider reported 0 cached prompt tokens. Repeated images get prefix-cache hits that make later
  calls faster, so cold latency is the fair measure of a new screenshot.

## Results
| format | bytes/image mean (KB) | click hit (1 img) | text exact (1 img) | text case-insens. | all (1 img) | all (3 img) | TTFT 1 img mean / p50 s | total 1 img mean s | TTFT 3 img mean / p50 s | total 3 img mean s | prompt tok/image | failed attempts (429/503, retried) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| png | 326.3 | 25/88 (28%) | 38/58 (66%) | 38/58 (66%) | 63/146 (43%) | 20/50 (40%) | 21.1 / 19.8 | 23.6 | 66.4 / 66.0 | 70.3 | 773.0 | 33 |
| jpg80 | 123.8 | 20/88 (23%) | 41/58 (71%) | 41/58 (71%) | 61/146 (42%) | 26/50 (52%) | 8.0 / 6.8 | 10.2 | 24.7 / 27.4 | 27.8 | 773.0 | 40 |
| jpg60 | 92.9 | 22/88 (25%) | 43/58 (74%) | 43/58 (74%) | 65/146 (45%) | 19/50 (38%) | 6.7 / 5.7 | 10.0 | 18.8 / 17.9 | 21.7 | 773.0 | 28 |
| jpg80half | 40.4 | 9/88 (10%) | 27/58 (47%) | 27/58 (47%) | 36/146 (25%) | 7/50 (14%) | 4.2 / 3.0 | 7.9 | 7.8 / 7.0 | 10.7 | 252.0 | 30 |
| webp80 | 54.2 | 18/88 (20%) | 37/58 (64%) | 37/58 (64%) | 55/146 (38%) | 18/50 (36%) | 4.6 / 3.6 | 6.9 | 12.5 / 13.0 | 16.2 | 773.0 | 40 |

Cold calls only (provider reported 0 cached prompt tokens, i.e. this image was not served from the prefix cache):
| format | n (1 img) | TTFT 1 img mean / p50 s | total 1 img mean s | n (3 img) | TTFT 3 img mean / p50 s | total 3 img mean s | output tok mean (1 img, mostly reasoning) |
|---|---|---|---|---|---|---|---|
| png | 70 | 21.4 / 19.7 | 23.9 | 24 | 64.8 / 64.4 | 68.7 | 404.5 |
| jpg80 | 74 | 8.5 / 7.0 | 10.7 | 21 | 25.3 / 28.5 | 28.6 | 387.7 |
| jpg60 | 73 | 7.0 / 6.2 | 11.2 | 24 | 17.2 / 13.7 | 19.9 | 535.5 |
| jpg80half | 43 | 3.9 / 3.2 | 6.2 | 9 | 6.7 / 7.6 | 9.1 | 585.0 |
| webp80 | 73 | 4.9 / 3.8 | 7.1 | 23 | 11.4 / 9.6 | 14.5 | 355.9 |

Paired vs PNG (same question, same image count, same repeat): format right & PNG wrong / format wrong & PNG right
- jpg80: n=196, +15 / -11
- jpg60: n=196, +18 / -17
- jpg80half: n=196, +7 / -47
- webp80: n=196, +11 / -21

Accuracy by app (1 image, all repeats):
| format | Chromium-browser | Gimp | libreoffice-calc | libreoffice-impress | libreoffice-writer |
|---|---|---|---|---|---|
| png | 5/8 | 12/18 | 20/50 | 9/30 | 17/40 |
| jpg80 | 6/8 | 12/18 | 18/50 | 10/30 | 15/40 |
| jpg60 | 6/8 | 10/18 | 22/50 | 10/30 | 17/40 |
| jpg80half | 5/8 | 5/18 | 10/50 | 6/30 | 10/40 |
| webp80 | 6/8 | 10/18 | 18/50 | 10/30 | 11/40 |

Sign tests on the paired rows (two-sided): jpg80 15/11 p=0.56, jpg60 18/17 p=1.0, jpg80half 7/47 p<1e-7, webp80 11/21 p=0.11.

## Findings
- **Latency follows bytes, not tokens.** All full-size formats cost the same 773 prompt tokens per image, yet cold TTFT
  ran PNG 21.4 s (326 KB), JPEG q80 8.5 s (124 KB), JPEG q60 7.0 s (93 KB), WebP 4.9 s (54 KB), half JPEG 3.9 s (40 KB).
  The cost is upload or decoding at the provider/relay, about 0.06 s per KB (inferred from this fit). With 3 images in context it multiplies
  (PNG 65 s). Pi resends every older screenshot each turn, so in the bench a PNG run spends most of its 480 s waiting.
- **Text reading** is not hurt by JPEG compression (q60 74%, q80 71%, PNG 66%). It is hurt by halving the size (47%).
- **Click grounding is weak for this model in every format** (20-28% hits at full size, 10% at half size). On targets further right, its x coordinates come out
  at about 0.6-0.8x the true value (e.g. Hyperlink at x 1009-1041 answered 715), while y is correct. That is a model trait,
  not a format effect. The agent should keep clicking by element_token, not by pixels.
- Limits: 15 shots, 12 of them LibreOffice. 2 repeats. Sampling noise is visible (repeat pairs disagree). Accuracy is a
  proxy for "can the model use the shot", not end-to-end task success. A 34-task run with the chosen format is the real check.

Files: `table.md`, `summary.json` (analyze.py), `questions.json` (targets + boxes in screenshot px), `results_main.jsonl`
(every call: answer, score, TTFT, total, usage incl. cached tokens), `results_probe.jsonl`, `groundtruth/` (gate
get_window_state metadata + AT-SPI dumps, no images), `images_jpg60/` (the 15 shots as JPEG q60 for reference; the original
PNGs and the other formats are on gui-pi under /data/workspace/shotexp/img), `scripts/`.
