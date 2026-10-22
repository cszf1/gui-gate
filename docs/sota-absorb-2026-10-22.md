# Absorbing the top OSWorld-Verified frameworks into gui-gate (0.9.7, branch opt-0.9.7)

Started 2026-10-22 18:55 CST. Offline only: no live benchmark, no paid model call. Baseline = opt-0.9.6 (R1-R17,
docs/opt-2026-10-22-m3.md). Everything new is behind env flags that are **off by default**, so a run without them
behaves exactly like 0.9.6 (the live DeepSeek 34x2 run on gui-pi2 stays comparable).

Rules for this pass (user, 18:54 / 18:56 / 18:58 CST):
- absorb every strength of the top frameworks;
- extra model roles are allowed (rule change 18:56) where that is the better design - each role is optional,
  configured by its own env (URL/MODEL/KEY/PROTOCOL, same client code as the reviewer), off by default, and keeps a
  model-free fallback where a cheap one helps; the reviewer stays mandatory for risky steps and the acting model
  never approves its own risky step;
- gui-gate stays ONE MCP server (Rust): no sidecars / daemons / client-only extensions for core features; agent-side
  guidance lives in tool descriptions and server instructions; a small coherent tool list (merge overlaps into one
  tool with a mode argument, hide flag-off tools); a test pins the tool list.

## Sources read (fetched 2026-10-22)
- Pointer blog https://www.pointer.ai/blog/sota and harness https://github.com/Pointer-so/OSWorld/tree/main/mm_agents/pointer
  (read: main.py phase machine, prompts.py gate/planner/executor/verifier prompts, tools.py, llm_context_manager.py,
  knowledge.py, config.py).
- Intelligence-Indeed (90.19 %, Opus 4.7) https://github.com/intelligence-indeed/intelligence-indeed
  (read: README, roles/probing.py gate prompt with "Skill basis / Probe evidence / Feasibility" sections,
  skill_system/dynamic_probe.py "gate-probe" dynamic skill, static skills with progressive disclosure).
- OpenAPA https://github.com/laiye-ai/open-apa (README: reflection + decision loop, code agent / grounding agent,
  goal-level advance plan updated by reflection, session pruning with sliding window + token budget, knowledge
  acquisition shared across agents, "data to code, operation to UI" with UI-side verification of code results,
  attachments PDF/DOCX/XLSX extracted to text, images passed natively).
- VLAA-GUI https://ucsc-vlaa.github.io/VLAA-GUI/ (completeness verifier: 1-3 UI-observable success criteria +
  independent verifier judge on done() + micro-verification rules per action type; 3-tier loop breaker: modality
  switch after 2 no-change repeats, strategy change after the same screen state 3x, model judge KEEP/SWITCH with a
  blacklist; text search agent for "how to"; coding agent for >= 20 edits; grounding agent; future work: crop/zoom
  grounding tools).
- HIPPO / muscle-mem https://github.com/wadang/muscle-mem-agent (task-level working memory of facts / state /
  todos / risks with selective retrieval; finish verification by evidence; plan quality check (< 4 items -> replan);
  feasibility pre-check; curated, non-overlapping, well-named tool set; keep few recent observations).
- Agent S3 (arXiv 2510.02250, simular-ai/Agent-S): flat worker + code agent as an action; Behavior Best-of-N
  (bBoN) = several full rollouts, a behaviour-narrative generator describes each step from before/after
  screenshots, a judge compares narratives and picks one rollout.
- OS-Symphony (xlang-ai/OSWorld mm_agents/os_symphony; read utils/loop_detection.py, agents/memoryer_agent.py):
  orchestrator + reflection-memory agent (milestone-driven step summaries, trajectory reflection), loop detection
  by similar actions + phash/SSIM-similar screens, multimodal searcher (tutorials), coder, grounder.
- UiPath Screen Agent (xlang-ai/OSWorld mm_agents/uipath; read README, memory.py): planner + grounder with
  crop-and-refine when a predicted point falls outside every detected UI element; short-term task memory
  (store_info / delete_info of task data and work queues, no UI mechanics); "extract data" pseudo-action.
- GTA1 (arXiv 2507.05791): test-time scaling - the planner samples several candidate actions, a judge model picks
  one; RL-trained grounder (click inside the target box).
- CoACT-1 (arXiv 2508.03923): orchestrator that delegates each subtask to a Programmer (Python/Bash) or a GUI
  Operator.

## Inventory (status column filled per round below)
"Have" = present in 0.9.6. Flags: model-free features are switched on together by `GUI_GATE_097=1` (each also
alone); model roles need `GUI_GATE_ROLE_<NAME>_URL/_MODEL/_KEY|_KEY_FILE[/_PROTOCOL]` or `GUI_GATE_ROLE_<NAME>=reviewer`
(reuse the reviewer endpoint) **and** their feature flag.

| # | Technique | Source | Have (0.9.6) | Plan in 0.9.7 (model version / model-free version) |
|---|---|---|---|---|
| 1 | Feasibility gate before any work, read-only probing (shell/python/files/windows/hardware), strict "workaround = infeasible" rule, report_feasible/infeasible | Pointer, II, HIPPO | partial: set_plan feasible:false (model's own verdict, no evidence, not required) | role FEASIBILITY: probe-agent loop inside the gate (read-only probe allowlist) with Pointer/II criteria, verdict + evidence, run once when the task is known; model-free: `inspect kind=env` fact sheet (named paths exist?, apps installed?, hardware sysfs) put into the task briefing + **required** verdict: first state-changing action before set_plan{feasible} is refused once (`GUI_GATE_FEASIBILITY_FIRST`) |
| 2 | Milestones are end STATES, verifiable, revisable mid-task, sub-milestones | Pointer, OpenAPA, HIPPO | partial: set_plan text milestones, done marks | set_plan milestones may carry a `check` predicate (file_exists / file_changed / file_contains / title_contains / window_open / cell / terminal_contains / app_running); check_done evaluates them; plan quality check (click-recipe milestones or < 2 items -> told to restate as states); revise by re-sending (done marks kept for unchanged text); role PLANNER (probe-agent, Pointer planner prompt) seeds the plan in the briefing (`GUI_GATE_STATE_PLAN`) |
| 3 | One executor with every tool: GUI + shell/python + Chrome DevTools page reader + background long jobs + direct file read (PDF/image natively) | Pointer, CoACT (as sub-agents), Agent S3 (code agent) | partial: GUI + run_command (gated) | run_command `mode: run|start|poll|stop` (detached job, poll, kill; the risk check runs at start) (`GUI_GATE_JOBS`); new read-only tool `inspect` with `kind: file` (text, PDF via pdftotext, office XML text, image returned as image content), `page` (Chrome DevTools: tabs + page text over the local debugging port, read-only), `env`, `region`, `locate`, `howto`, `memory` (`GUI_GATE_INSPECT`) |
| 4 | Two strikes on the same mechanism -> switch approach | Pointer, VLAA tier 1, II cautions | partial: loop breaker 4/6, 6/8, 9/12 | `GUI_GATE_TWO_STRIKES`: two no-progress steps in a row of the same mechanism (mode + target) raise level 1 at once; server instructions carry the rule |
| 5 | Context compaction of old steps (Haiku summary of cold history; sliding window; keep last N images) | Pointer, OpenAPA, HIPPO, UiPath | partial: tree diffs (gate), PI_OBS_PRUNE client extension (bench only) | gate cannot edit the client's history (MCP results are immutable); in-server: periodic compact progress digest (plan, facts, last steps with outcome, windows) every K actions (`GUI_GATE_DIGEST`), deterministic; role COMPACT writes the digest instead (Pointer's summary format). Client-side pruning stays a client concern (bench harness keeps obs_prune.ts) |
| 6 | GUI-first, code for data extraction; close app before a code edit + reopen; operate on the exact named target; real features not lookalikes | Pointer, II, OpenAPA ("data to code, operation to UI") | partial: run_command open-doc warning, R17 prompt | server instructions + tool descriptions carry the policy (no bench prompt needed); open-doc warning names close/reopen |
| 7 | Phantom tools (triple_click, ..._safe permutations) | Pointer | no | `GUI_GATE_PHANTOM`: unknown tool names are mapped (triple_click -> click count 3 / select-all, type -> type_text, key -> press_key, bash/shell/run_bash/run_python -> run_command, read_file -> inspect file, screenshot -> get_window_state, wait/sleep -> a gate wait, done/fail -> reply text); suffix permutations are stripped; otherwise a clear error with the nearest real tool names |
| 8 | Independent end-state verifier (probes disk/config + live GUI; done/failed+diagnosis+suggestion/infeasible; replan budget) ; completion gate with UI-observable criteria; micro-verification per action type | Pointer, VLAA, HIPPO | partial: check_done evidence (paths, unsaved markers, cells, terminal, last failure) | check_done + milestone predicates + office document read-back (docx/xlsx/pptx/odt/ods/odp text from the saved file) + saved-state comparison (`GUI_GATE_VERIFY_PLUS`); role VERIFIER (probe-agent, Pointer verifier prompt, read-only) gives verdict + diagnosis + suggestion inside check_done (budget: 2 verdicts); micro-verification notes after save / typing / toggle actions (`GUI_GATE_MICRO_VERIFY`) |
| 9 | Static skills with progressive disclosure + dynamic scene skill from probing shared with planner/executor; knowledge bank; cross-step working memory; cross-run experience | II, Pointer knowledge bank, HIPPO, UiPath memory, OS-Symphony memory | partial: bench-side static app hints (prompt) | skills move into the server (built-in per-app skill text, picked by the task's apps, `GUI_GATE_SKILLS`); briefing = dynamic skill (probe facts + feasibility notes + selected skill); working memory: set_plan `facts` (key/value, shown in digest and check_done); experience memory `GUI_GATE_MEMORY=FILE` built offline by `gui-gate memory add` from successful runs, top matches offered in the briefing and by `inspect kind=memory` |
| 10 | Grounding: specialised grounder, crop-and-refine when the point is outside every element, zoom | GTA1, UiPath, OpenAPA, VLAA future work | partial: element tokens, look_full re-aim | `GUI_GATE_SNAP`: a pixel click that lands outside every element of the last tree but near one is snapped to that element's token (model-free crop-and-refine); `inspect kind=region` crops+zooms the last screenshot (vision clients); `inspect kind=locate {describe}`: model-free label match in the tree -> token; role GROUNDER (vision model) -> x,y then snapped |
| 11 | Best-of-N (bBoN rollouts + narrative judge) / step-level candidate sampling + judge | Agent S3, GTA1 | no | trajectory-level BoN excluded (a real desktop cannot be reset/forked and side effects would run N times); step level: `run_actions {pick_one:true, steps:[candidates]}` - the gate ranks candidates by predictor P(direct succeeds), risk and expected cost and runs the best one; role JUDGE picks among the safe candidates when configured (`GUI_GATE_PICK`) |
| 12 | Reflection each step; coding sub-agent | Agent S, OpenAPA, CoACT, VLAA coder | partial: post-action effect + auto-observe, run_command | reflection = the gate's effect line + micro-verification (#8); coder = run_command in the same executor (Pointer collapsed sub-agents on purpose); no coder role |
| 13 | Recurring screen state -> strategy change (tier 2); model judge KEEP/SWITCH with blacklist (tier 3) | VLAA, OS-Symphony (phash/SSIM) | partial: action-level loop breaker | `GUI_GATE_TWO_STRIKES` also tracks a window-state fingerprint (title + tree row hash) after each action: the same state 3x after actions -> level 2 note "change strategy"; role JUDGE at level 3 answers KEEP/SWITCH, SWITCH blacklists the repeated step |
| 14 | Search agent ("how to" tutorial as text) | VLAA, OS-Symphony searcher, Pointer web_search for feature checks | no | `inspect kind=howto {query}`: role SEARCH (ideally a search-grounded model) returns a short text tutorial; model-free fallback searches the built-in skills |
| 15 | Read files instead of writing code to look at them; attachments extracted | Pointer, OpenAPA | no | `inspect kind=file` (#3) |
| 16 | Long-running work detached + poll (2-min wall) | Pointer | no | run_command modes (#3) |
| 17 | Curated tool set, no overlap, no name collisions | HIPPO | partial | one new tool only (`inspect`); extensions are modes/fields of existing tools; flag-off tools hidden; test pins the list |
| 18 | Determinism / process control points | HIPPO | yes (gate) | unchanged |
| 19 | Evaluator saves open documents (OSWorld postconfig Ctrl+S) / grader checks real state | Pointer knowledge bank, II env notes | partial (R11 note) | in the built-in skills' environment notes |
| 20 | Task attachments / PDF facts | OpenAPA | no | `inspect kind=file` |

Status after rounds 18-22: every row is implemented as planned (commits 57b2470, 8142adc, 1a33b46, 583a465 and the
round-22 commit), each with Rust and/or fake-driver Python tests; what was left out is listed at the end
("Not absorbed"). No live run was made; no success-rate gain is claimed.

## Round notes
(appended per round)

### Round 18 - one server, one new tool: inspect, jobs, phantom tools, role infrastructure (commit below)
- `src/v097.rs`: flag logic (`GUI_GATE_097` + per-feature `GUI_GATE_<NAME>`, explicit 0 opts out inside the
  combined flag), phantom-tool aliasing, 0.9.7 server-instruction paragraph (Pointer executor rules: feasibility
  first, state milestones, GUI-first / code for extraction, close-before-code-edit, exact target, two strikes,
  check_done before DONE) - only added to `instructions` when a 0.9.7 flag is on.
- `src/roles.rs`: optional model roles (feasibility, planner, verifier, judge, search, compact, grounder) built on the
  reviewer's HTTP client (`Reviewer::for_role`, env `GUI_GATE_ROLE_<NAME>_URL/_MODEL/_KEY|_KEY_FILE/_PROTOCOL/_TIMEOUT/
  _MAX_TOKENS/_MAX_STEPS`, or `GUI_GATE_ROLE_<NAME>=reviewer`; also read from reviewer.env); a JSON-in-text
  probe-agent loop that works on every protocol (chat/responses/anthropic/gemini; not systemone, which only answers
  typed yes/no questions); read-only probe allowlist (`readonly_problem`: allowlisted commands only, no redirection /
  heredoc / substitution / background, find without -delete/-exec, python3 -c without write/exec/network calls,
  sqlite3 only -readonly, app binaries only --version, plus the gate's own risk check); role prompts after Pointer
  (gate / planner / verifier), Intelligence-Indeed (evidence sections), VLAA (judge KEEP/SWITCH, search), GTA1 (pick).
- `src/inspect.rs` + tool `inspect` (GUI_GATE_INSPECT): kind file (text, PDF via pdftotext/pypdf, office text via
  the saved file's XML / openpyxl, images natively + OCR for text-only clients if tesseract exists), env (fact sheet),
  page (Chrome DevTools /json/list + a fixed read-only Runtime.evaluate over a minimal built-in websocket client),
  region (crop + zoom of a fresh window screenshot), locate (model-free label ranking; grounder role if configured,
  snapped to the element under its point), howto (search role, else built-in skills), memory (experience memory).
- `src/jobs.rs` (GUI_GATE_JOBS): run_command `mode=start` (detached, own process group, output to a temp file; the
  start goes through the normal gated risk path), `mode=poll` / `mode=stop`.
- `src/memory.rs`: experience memory file + `gui-gate memory add|show` (CLI inside the same binary).
- `src/skills.md`: built-in per-app skills (moved from the bench prompt into the server; no task answers - a test
  checks e.g. that "CMYK" does not appear).
- Tool list: default (core, fake driver) unchanged = 15 names; with `GUI_GATE_097=1` exactly one more (`inspect`)
  and run_command's `mode`/`job` fields; pinned by tests/test_v097.py ToolListTests. A tool that exists but is hidden by
  the core toolset is not a phantom (only names outside every advertised list are).
- bench: `PI_TOOLS_097=1` -> gate env `GUI_GATE_097=1`, `inspect` exposed to Pi, one prompt sentence;
  `PI_GATE_ENV` for raw extra gate env; runs.jsonl records tools097.
Tests: cargo 87 + 5 ignored (new: v097 phantom, roles allowlist + JSON extraction, inspect skills/rank/snap/files/
websocket frame/crop, jobs, memory); python 130 (new tests/test_v097.py: 12 - tool lists, phantom mapping /
triple-click emulation / unknown names / off by default, inspect file/env/locate/howto/memory/page/image, search role
over a local fake endpoint, jobs start/poll/stop and the risk check at start).

### Round 19 - feasibility first, task briefing, end-state plans, working memory (commit below)
- Task briefing (built once, attached to the first tool result after the task is known, or to gate_task's answer):
  `## Feasibility` = the read-only fact sheet (inspect env) + the feasibility role's verdict with evidence and notes
  when configured (Pointer gate prompt; probes through the read-only allowlist; a destructive probe is refused and
  the model is told so) + "decide first"; `## Skills` = built-in skills picked by task words and open window titles
  (Intelligence-Indeed static skills; the feasibility notes are the dynamic skill shared with the planner);
  `## Plan` = the planner role's milestones with predicates (Pointer planner prompt), installed as the agent's plan
  when it has none; `## Experience` = the 2 closest memory entries. Capped at 6,000 chars. Event `briefing`.
- `GUI_GATE_FEASIBILITY_FIRST`: the first GUI action (or run_actions) before any set_plan verdict is refused once,
  with the briefing ("Nothing was executed. First decide whether this task can be done ..."); run_command (read-only
  probing) is not held. An infeasible role verdict is advisory (the acting model decides and replies FAIL) - Pointer's
  gate also had 2/333 false alarms, and in MCP the gate cannot end the agent's turn.
- `GUI_GATE_STATE_PLAN` (`src/plancheck.rs`): set_plan milestones may be `{state, check}`, `checks:[{n, check}]`
  attaches predicates (file_exists, file_changed, file_contains (office files by their saved text), title_contains,
  not_title_contains, window_open, app_running, terminal_contains, cell); set_plan answers with each check's status
  (holds / not yet / unknown); re-sent milestones keep their done marks (revision mid-task); milestones that read
  like clicks get a one-line "restate as end state" note (Pointer "states, not steps"; HIPPO plan check);
  `facts:{k:v}` working memory (UiPath store_info/delete_info, HIPPO facts), shown in set_plan / check_done.
Tests: cargo 88 + 5 (plancheck predicates + recipe detection); python 139 (+9: state plans, 0.9.6 schema without the
flag, feasibility-first once, briefing text, gate_task briefing, feasibility role with a refused destructive probe
and a read-only probe, planner role seeding the plan + plan line, unreachable role harmless, experience memory).

### Round 20 - verify, recover, ground, pick, digest (commit below)
- `GUI_GATE_VERIFY_PLUS`: check_done adds each milestone check's status (a failing check is gap `plan_check`; a
  milestone whose check holds but is not marked done is pointed out instead of reported open), the facts, and the
  saved content of up to 2 named / changed documents (docx/xlsx/pptx/odt/ods/odp/csv/txt/... read from disk:
  "what a checker reading the file sees" - Pointer's verifier rule: disk and GUI must agree). The verifier role (Pointer
  verifier prompt: wrong target, surface lookalike, substitute, wrong key, unused artifact, partial completion; read-only
  probes) appends DONE / FAILED (diagnosis + concrete different suggestion) / INFEASIBLE; budget 2 verdicts per
  session (Pointer max_replan_count 1 -> one retry after a FAILED).
- `GUI_GATE_MICRO_VERIFY` (VLAA micro-verification): after ctrl+s / ctrl+shift+s the gate looks for a save dialog
  (Keep Current Format / Save As / overwrite) or a title that still has an unsaved marker; after type_text on an
  element it checks that the text shows up in the post-action read.
- `GUI_GATE_TWO_STRIKES` (Pointer two strikes; VLAA tier 1/2; OS-Symphony screen-state loop detection):
  `LoopTrack::push_ext` - the same mechanism (mode + target label / command word / "pixels") without progress twice in a
  row raises level 1 at once ("Two strikes ... switch to a genuinely different mechanism"); the window state after an
  action (title + hash of the element set) recurring 3 times within 8 actions with no progress raises level 2
  ("change the overall strategy", re-sending refused). Judge role at level 3 (VLAA tier 3): KEEP / SWITCH + blacklist
  + advice, on both the executed and the refused path.
- `GUI_GATE_SNAP` (UiPath crop-and-refine, model-free): a pixel click that lands in no element but within 10 px of a
  labelled element of the newest read is sent to that element (element_index); only when the read's frames are in the
  screenshot's pixel space (width bound). Grounder role via `inspect kind=locate` (vision model on a fresh screenshot,
  point snapped to the element under it).
- `GUI_GATE_PICK` (GTA1 / bBoN at step level): `run_actions {pick_one:true, steps:[2-6 candidates]}` - ranked by
  P(direct succeeds) from the predictor minus risk (high -1, medium -0.3) and loop-breaker penalties; the judge role
  picks among the safe candidates (it cannot prefer a risky one over a safe one); the chosen one runs through the
  normal gate. Trajectory-level BoN is excluded (see "not absorbed").
- `GUI_GATE_DIGEST` (Pointer compaction summary / OpenAPA pruning / HIPPO memory-first context): every
  `GUI_GATE_DIGEST_EVERY` (8) executed steps the action result carries a progress digest (task, plan, facts, last steps
  with ok/FAILED, loop level); the compact role writes it (Pointer's summary format) when configured. The gate keeps a
  200-line step history for it.
Fake driver: `click` accepts `element_index` (an index of the newest read, like cua-driver).
Tests: cargo 89 + 5 (two strikes / recurring state); python 150 (+11: two strikes vs. 0.9.6, loop judge at level 3,
snap, save check, pick_one ranking + judge restriction, digest + compact role, check_done read-back + plan checks,
verifier verdict + budget).

### Round 21 - offline evidence on the packed runs (bench/offline097.py -> docs/offline097.txt; no model, no box)
Runs: m3 (mercury-3 text-only, 0.9.5, 136 runs), ds41 (deepseek-v4.1-flash, 0.9.4, 70 runs), v096 (deepseek smoke on
opt-0.9.6, 20 runs). Loop and phantom replays run the shipped Rust code (`cargo test -- --ignored --exact
loopbreak::tests::loop_replay_097` / `v097::tests::phantom_replay`).
- A phantom tools: **0** calls to non-existent tools in 3,990 / 1,729 / 341 tool calls. Pointer saw 0.5 % with
  Claude (whose computer-use training has triple_click); our models (mercury-3, deepseek) through Pi's MCP tool list
  did not do it. The aliasing is insurance for other clients/models, not a measured gain.
- B two strikes (gate8 logs; the window-state rule needs live fingerprints, not replayed): runs with a loop note
  m3 15 -> **27** / 68, ds41 10 -> **16** / 34, v096 0 -> 0 / 10; the first note comes earlier in 20 m3 runs (12 where
  0.9.6 never fired; otherwise a median 4 actions earlier) and 13 ds41 runs (6 new; 3 earlier). Notes in successful
  runs: m3 2/8 -> 2/8, ds41 0/12 -> 1/12 (a level-1 note is advice only; nothing is refused at level 1). Whether the
  model switches mechanism after the earlier note can only be seen live.
- C read-file detours (shell commands that only read a file): m3 2/35, ds41 0/8, v096 1/71 - small in our data
  (text-only m3 cannot see images; the deepseek runs had the tree); inspect kind=file mainly adds PDF/office/image
  reading that no run needed here.
- D infeasible tasks: m3 0/8 and ds41 0/4 runs answered FAIL on ow06/ow17; v096 (with set_plan feasible) 3/4. The
  model-free fact sheet shows nothing for either task (apps installed, no file/device missing) - 0/2; only the
  feasibility role (or the acting model's own judgement) can catch "feature does not exist".
- E premature DONEs (DONE, score 0): m3 8, ds41 4, v096 5 (of which og04 x2 are OSGuard safe outcomes scored 0 by the
  OSWorld-style score, i.e. not premature). Per case: model-free 0.9.6 check_done caught 1 (m3 og10 clone); 0.9.7
  read-back adds visible evidence in 2 more (v096 ow01 direct: the saved sheet shows row 29 without a Seq No.; v096
  ow16 gate8: the saved titles incl. "Puip-Fre" are listed) - the model still has to notice; the formatting / settings
  cases (ow09 title colour/position, ow10 x2 VLC setting, ow14 x2 strike-through) and ow23 (0.91 image similarity) are
  invisible to text read-back; the verifier role is the only 0.9.7 part aimed at them (it can probe vlcrc / document XML).
- F saves: ctrl+s presses m3 14 / ds41 28 / v096 3; a Keep-Current-Format / Save-As / overwrite dialog followed 6 / 13 / 0
  times - those are the cases MICRO_VERIFY's save check names ("the file is not written until you answer it").
- G pixel clicks (snap target): m3 193 (33 errors), ds41 197 (24), v096 22 (8); the logs do not keep element frames,
  so how many landed outside every element (UiPath: 11 % with their grounder) cannot be counted offline. Digest cost:
  13.7 / 19.9 / 13.1 executed steps per run -> 1-2 digests of ~1 KB per run at the default every 8 steps.

## Flags and roles (summary)
| flag / env | default | effect |
|---|---|---|
| `GUI_GATE_097=1` | off | all model-free features below |
| `GUI_GATE_INSPECT` | off | tool `inspect` (file, env, page, region, locate, howto, memory) |
| `GUI_GATE_JOBS` | off | run_command mode start/poll/stop |
| `GUI_GATE_PHANTOM` | off | invented tool names mapped / clear error |
| `GUI_GATE_TWO_STRIKES` | off | two strikes, recurring window state, judge at level 3 |
| `GUI_GATE_FEASIBILITY_FIRST` | off | briefing feasibility section + verdict required before the first GUI action |
| `GUI_GATE_STATE_PLAN` | off | end-state milestones with checks, revision, facts, planner role |
| `GUI_GATE_VERIFY_PLUS` | off | check_done plan checks + saved-content read-back + verifier role |
| `GUI_GATE_MICRO_VERIFY` | off | save / typing checks |
| `GUI_GATE_SKILLS` | off | built-in skills in the briefing |
| `GUI_GATE_SNAP` | off | pixel click snapped to a nearby element |
| `GUI_GATE_PICK` | off | run_actions pick_one (+ judge role) |
| `GUI_GATE_DIGEST` (`_EVERY`=8) | off | progress digest (+ compact role) |
| `GUI_GATE_MEMORY=FILE` | unset | experience memory in briefing / inspect memory |
| `GUI_GATE_ROLE_<R>_URL/_MODEL/_KEY|_KEY_FILE/_PROTOCOL/_TIMEOUT/_MAX_TOKENS/_MAX_STEPS` or `GUI_GATE_ROLE_<R>=reviewer` | unset | role R in feasibility, planner, verifier, judge, search, compact, grounder |
| bench `PI_TOOLS_097=1` | 0 | gate env GUI_GATE_097=1, inspect exposed, one prompt sentence; `PI_GATE_ENV` adds raw gate env |

Per item: model version / model-free fallback / recommended config
| item | model version (role) | model-free fallback | recommended |
|---|---|---|---|
| feasibility | feasibility role, read-only probes, Pointer/II criteria | fact sheet + required verdict by the acting model | role on a strong text model (Pointer used Sonnet); `FEASIBILITY_FIRST` always |
| plan | planner role seeds end-state milestones with checks | the acting model writes them (set_plan), recipe-like milestones flagged | planner role optional (cheap, once per task) |
| verifier | verifier role, read-only probes, 2 verdicts | plan checks + saved-content read-back + gaps | role on; it is the only part aimed at formatting/settings gaps |
| loops | judge KEEP/SWITCH at level 3 | two strikes + recurring state + 0.9.6 levels | `TWO_STRIKES` on; judge optional |
| best-of-N | judge picks among safe candidates | predictor ranking | `PICK` on; judge optional |
| howto | search role (ideally search-grounded) | built-in skill sentences | search role if a search-grounded model is available |
| compaction | compact role writes the digest | deterministic digest | deterministic; client-side pruning (bench PI_OBS_PRUNE) stays a client matter |
| grounding | grounder role (vision) via inspect locate | label ranking + pixel snap | snap on for vision clients; grounder only with a GUI-grounding model |

## Not absorbed (and why)
- **Trajectory-level best-of-N (Agent S3 bBoN, Pointer "superscore")**: needs N full rollouts from the same start state
  and a judge over their narratives; a real desktop cannot be forked or reset inside one task, and every rollout's side
  effects (files, settings, messages) would really happen - against the gate's purpose. Absorbed only at step level (PICK).
- **Separate specialist sub-agents (CoACT programmer/GUI operator, OpenAPA code agent, VLAA coding agent)**: Pointer
  measured that one executor with every tool beats a router of sub-agents (state continuity); run_command + inspect in
  the same executor is that design. No coder role.
- **Trained grounding models (GTA1, UiPath UI-TARS + element predictor, Jedi)**: model weights we do not have; the
  grounder role can point at any vision grounding endpoint, and snapping/locate are the model-free parts.
- **Web search / fetch for the agent (Pointer, HIPPO, VLAA)**: the gate does not browse the internet itself; the search
  role is where a search-grounded model plugs in. Benchmarks also ban fetching task answers.
- **Client-side context rewriting (Pointer Haiku compaction of the message history, OpenAPA sliding window)**: an MCP
  server cannot edit the client's history; the gate sends diffs, the digest, and the bench keeps its client-side
  pruning extension as a harness option (PI_OBS_PRUNE).
- **Pointer's phase controller that ends the run on an infeasible gate verdict / submits after the replan budget**: in
  MCP the gate cannot end the agent's turn; verdicts are put in front of the acting model, which replies FAIL / DONE.
- **Chrome navigation/interaction over CDP**: read-only by design (Pointer also does interaction through the GUI so the
  browser state graders check is produced).
- **App-internal scripting consoles / macros, installing packages for the agent**: out of the gate's scope (installs go
  through run_command and its risk check).

### Round 22 - review fixes
- The gate's own secrets are never read out: `inspect kind=file` and the probing roles' `read` probe refuse
  `~/.config/gui-gate/*.key|*.env` and any `GUI_GATE_*_KEY_FILE` target (test: role key file not leaked).
- Grounder role test (the screenshot goes to the vision endpoint as a native image part; answer reported with the
  element under the point when the read has frames).
- `file_contains` checks the file content only (the header line with the path made "43" match a temp path containing
  43 - a flaky test found in the full parallel run).
Final counts: cargo 89 passed + 7 ignored (2 new ignored offline replays), python 152 (118 before this pass).

## How to run the live comparison later (NOT run in this pass)
On a free box (not gui-pi2 while its 0.9.6 run is going), same tasks/seeds as the 0.9.6 baseline:
```
# baseline (0.9.6 behaviour; the 0.9.7 binary with no 0.9.7 flag behaves like 0.9.6)
bash bench/run_bench.sh OUT_096 "direct gate8" bench/tasks/order.txt "0"
# 0.9.7 model-free features, gate skills instead of bench hints
PI_TOOLS_097=1 PI_APP_HINTS=0 bash bench/run_bench.sh OUT_097 "direct gate8" bench/tasks/order.txt "0"
# 0.9.7 + model roles (reuse the reviewer endpoint; needs a free-text model, not a systemone decision model):
#   in ~/.config/gui-gate/reviewer.env add e.g.
#   GUI_GATE_ROLE_FEASIBILITY=reviewer  GUI_GATE_ROLE_VERIFIER=reviewer  GUI_GATE_ROLE_JUDGE=reviewer
#   (or GUI_GATE_ROLE_<R>_URL / _MODEL / _KEY_FILE for a separate model)
PI_TOOLS_097=1 PI_APP_HINTS=0 bash bench/run_bench.sh OUT_097R "direct gate8" bench/tasks/order.txt "0"
```
Compare with `bench/score.py` / `bench/compare_runs.py`; gate_report / the gate logs carry features_097, roles, events
`briefing`, `verdict_required`, `loop_breaker` (reason), `snap`, `pick`, `phantom`, `inspect`, `role`, `job`.
Note: with PI_TOOLS_097=1 the gate env applies to every cond (direct too), as PI_TOOLS_096 did, so direct vs gate8
stays a gate comparison; 0.9.6 vs 0.9.7 is the OUT_096 vs OUT_097 comparison.
