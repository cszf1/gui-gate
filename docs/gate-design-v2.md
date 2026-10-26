# gui-gate 门控设计 v2（骨架）

状态：v2 草稿，分支 `v2-hum`（基于 main c3893bc）。本文是整体设计的骨架：架构与信任边界、决策流程、每种门动作的触发条件、
问人协议状态机、安全不变量、已知限制、待在线验证的假设。价值模型（数学部分）单列在 `docs/value-model-v2.md`，由另一位同事填写。
细节以代码为准；引用的函数都在 `src/` 下。

相关文档：`docs/security-model.md`（安全边界的逐条说明）、`docs/mcp-2026-07-28.md`（MCP 2026-07-28 / MRTR 线上格式）、
`docs/decision-v2.md`（v2 决策的历史说明）、`docs/value-model.md`（当前价值模型）。

---

## 1. 架构与信任边界

```
 用户 ──(桌面壳的原生窗口 / 任务文件)──┐
                                        │ 可信通道
 模型(Pi) ──tools/call──> gui-pi 扩展 ──MCP 2026-07-28──> gui-gate ──MCP──> cua-driver ──> 桌面应用
                ▲                          │   (Rust，安全核心)
                └──── 工具结果（不可信方向）─┘   └──> 审核模型 API（独立模型，非 agent 的模型）
```

| 组件 | 信任 | 说明 |
|---|---|---|
| 用户 | 可信 | 只通过两条路影响授权：任务文件（`--task-file`，壳写，`gui-pi-task/1`）和问人表单的回答。 |
| 桌面壳（gui-pi-desktop） | 可信 | 写任务文件；渲染问人表单（MRTR `elicitation/create`）；持有密钥，经 person IPC 交给门一次。 |
| gui-pi 扩展 | 半可信（协议搬运） | 只搬运 `inputRequests` / `inputResponses` / `requestState`，不能自己构造“同意”（`requestState` 由门签名，见 4.3）。 |
| 模型（agent） | **不可信**（威胁模型：合作但会出错，不是恶意） | 能调工具、写参数、调 `gate_task`；它说的话永远不授权任何风险步骤。 |
| gui-gate | 安全核心 | 所有动作都经过它；风险判断、批准、派发许可都在这里。 |
| 审核模型 | 半可信（独立判断） | 只看到用户原话、具体动作、窗口树；看不到 agent 的解释。它的原话不回给 agent（见 5、6）。 |
| cua-driver | 执行器 | 门只信它返回的观察，不信其中的文字（应用文字可能模仿门的消息，见 6.4）。 |
| 应用 / 网页 / 终端文字 | **不可信** | 进入 agent 上下文前做 `util::defang`。 |

明确不在边界内的（见第 8 节）：与门同用户的任意代码执行（`run_command` 能读写同用户文件，包括门日志）、恶意模型。

## 2. 决策流程

入口：`Gate::handle_call`（每个 `tools/call` 一个线程）→ `handle_call_inner` → 按工具分派；GUI 动作走 `Gate::gated` /
`gated_inner`，批量走 `batch::run_actions`（每步仍调 `gated`），透传动作走 `pass_action`，命令走 `exec_command`。

`handle_call` 在分派前做三件事：

1. `sweep_person_requests`：把过了 deadline + 5 s 还没重试的 MRTR 请求各记一条 `human_outcome`（`no_retry`）。
2. 若带 `requestState`：`PendingTable::resolve` 验签、一次性消费、绑定检查，得到人的回答（或 Late = 超时），放进本线程的 `CallCtx`。
   验证失败 → JSON-RPC -32602，什么都不做。
3. 冻结：有问人请求开着（`Human::person_answering`）时，所有动作类调用（`is_action_call`）直接拒绝（`frozen_result`）。

`gated_inner` 的顺序（一步动作）：

```
build_ctx → features
  │
  ├─ 上一次同一动作 outcome unknown → look_light（先读窗口，绝不盲发）
  ├─ 非风险步骤：policy_decision（基线 direct/light/full）
  └─ 否则：
       hard_rules (decide.rs)            零成本规则：handoff / 风险 / 陈旧 token / 无观察 / 盲重发 / 循环 / 新窗口
         └─ 风险 → risk_gate (server.rs)  拒绝记录 → 启动者任务授权(authz::authorized) → 审核模型 → 人
              └─ 授权通过 → 再跑一次 hard_rules(confirmed=true)，剩余零成本规则仍生效
       choose (decide.rs → value.rs)      价值模型：最小期望代价（见第 4 节标题，正文在 value-model-v2.md）
  │
  ├─ safety_decision_ok 断言：风险/接管步骤只能以 ask_human / handoff / deny / risk_review / stop_loop 结束（除非可信来源已批准）
  ├─ fast_path / 批量内 / 换模式 的放行（只对非风险、软决策）
  ├─ check / wait / heal 在门内解决（各至多一次）
  ├─ reason 已用过 → direct；review → review_step（审核模型看非风险的可疑步骤）
  ├─ loop breaker level ≥ 2 → stop_loop
  ├─ ask_human / handoff / ask_help 且有人可问 → Human::ask_kind（第 5 节）
  └─ direct → grant（一次性派发许可）→ Gate::tool → dispatch_check / own_dialog_check / 冻结 → 驱动
     其它 → refusal（结构化拒绝，附自动观察）
```

最终派发点只有一个：`Gate::tool`。所有发往驱动的状态改变（门内的重发、遮挡处理、批量步骤、清终端行）都在这里重建 `ActionCtx`，
风险/接管目标必须有 `grant` 签发、绑定最终参数签名和效果键的一次性许可（`dispatch_check`）。

## 3. 每种门动作的触发条件

| 动作 | 触发（谁决定） | agent 看到什么 | 执行了吗 |
|---|---|---|---|
| `direct` | 价值模型最优；或 fast_path / 批量内 / 授权通过 / 人批准 / 审核允许 | 执行结果 + 自动观察（树 diff） | 是 |
| `check` | 价值模型（信息价值）；门内树探测，至多一次 | —（门内解决后重新决策） | 视后续 |
| `wait` | 加载提示 / 弹窗刚出现；门内等 ≤1.5 s，至多一次 | — | 视后续 |
| `heal` | 陈旧 token（hard_rules）；按 role+label 重新绑定；绑到风险元素 → look_light | — | 视后续 |
| `look_light` | 无观察、新窗口、unknown 后、check/wait 解决不了、审核怀疑 | `[gate] Not executed` + 附新树 | 否 |
| `look_full` | 像素目标且窗口变了、同一动作刚失败、价值模型 | 同上 + 截图 | 否 |
| `reason`（extra_reason） | 价值模型；同一步只给一次 | `next_action: rethink`，`reasoning_budget_tokens` | 否 |
| `review` | 同一动作重复且无确认效果（可能循环）；审核模型看目标 | 允许 → 执行；否则 look_light（“reviewer doubts …”） | 视审核 |
| `risk_review` | 中风险且启动者任务未授权（`risk_mode=auto`） | 审核允许 → 执行；拒绝 → deny；审核没答 → ask_human | 视审核 |
| `ask_human` | 高风险未授权（或 `risk_mode=human`），或审核没答 | 人批准 → 执行一次；否则拒绝 + 安全替代 | 视人 |
| `handoff`（takeover） | 必须人做的步骤（密码框等，`ctx.handoff`） | 人做完 → `status: done_by_person`，门不再发原动作 | 否（人做） |
| `ask_help` | 连续失败、价值模型认为问人最便宜，且有人可问 | 人代做 → `done_by_person`；“换个办法”/提示 → 拒绝 + 提示 | 否 |
| `stop_loop` | loop breaker level ≥ 2：同一步在本轮循环里已无进展 | “换个明显不同的办法” | 否 |
| `deny` | 拒绝记录覆盖同一效果、审核拒绝、无审核配置、无人可问 | `Not executed` + 不要换路达到同一效果 + 安全替代 | 否 |
| `freeze` | 有问人请求开着 | “有人正在回答” | 否 |
| `own_dialog` | 目标是门自己的对话框进程/窗口 | 拒绝 | 否 |

`structuredContent.status`：`executed | blocked | partial | done | unknown | input_required | done_by_person`（`set_status`）。
客户端按 status 而不是 `[gate]` 文本前缀判断。

## 4. 价值模型（摘要；全文见 docs/value-model-v2.md）

接口不变：`decide::choose(ctx, f, cfg, check_available, check_done) -> Decision`；安全必须问的步骤从不进入价值比较
（`safety_decision_ok` 断言）。数字来自 `results/offline-v2-2026-10-26/`（gui-pi3 OSWorld-Verified 一次运行，332 个有效任务，只读离线分析）。

**开关与默认。**

- `GUI_GATE_VALUE_MODEL` 默认 **`legacy`**（手设先验，决策与 c3893bc 完全一致，golden 测试 `src/testdata/value_default_golden.json`，
  189 个决策）。`treatment` / `auto` 需显式打开；只加载 `GUI_GATE_VALUE_WEIGHTS` 不再切换模型。
- `GUI_GATE_VALUE_MODEL=v2`：`run_command` 不进 GUI 预测器（`tier: command`，`p_direct: null`）；GUI 历史特征只算 GUI 动作；
  新增 click/scroll/role 等特征（`features::V2_FEATURES`）；只有 `treatment_v2` 权重且区间可用时才用治疗模型。
- 无 bootstrap 视为不可用：模型文件 bootstrap 次数 < 20 时区间为未知（不再是宽度为 0 的点区间），该模型不被使用，回退 legacy 先验，
  值日志记 `fallback`。

**校准（命令与 GUI 分开）。** 内置 osworld_v3 预测器：GUI AUC 0.694 / ECE 0.043；命令 AUC 0.543 / ECE 0.674；两者混合 0.617 / 0.202
（2026-10-25 的“0.626 / 0.197”主要是命令行拉低的）。按留出域（leave-one-domain-out）重拟合：v1 特征 GUI AUC 0.773，v2 新特征 0.770，
**无增益**（差 −0.003，配对 bootstrap 95 % CI [−0.018, +0.012]）。

**代价。** 从门日志 + Pi 日志重估：look_full 实测平均 26.4 s（假设 9 s），review 15.8 s（假设 3 s），两者被明显低估；
look_light、wait、check、heal、ask 与假设大致相符。默认代价表未改。

**能估什么、不能估什么。**

- 重叠（IPW 支持度）：只有 `direct` 和 `wait` 有足够重叠；look / review / check / heal 在日志策略下几乎是确定性的，观测数据无法给出其效果。
- “干预避免了失败”**不可从日志估计**（被拒步骤没执行，反事实结果不可观测），不作此声明；能测的只是“执行失败的事先预测”。
- 在线方案（`bench/online_plan.py`，`results/offline-v2-2026-10-26/online_plan.json`）：108 个留出任务 × 2 臂（legacy vs v2）= 216 次运行，
  按任务成对、顺序随机、预注册分析；**未执行**。

## 5. 问人协议状态机

### 5.1 通道

`Human::resolve_channel`：`elicit`（2026-07-28 客户端走 MRTR；老客户端走服务端 `elicitation/create`）→ `desktop`（本机原生对话框）
→ `chat`（门联系不到人：拒绝，让 agent 去问它的用户）；`deny` = 无人值守（每个请求立即拒绝，`outcome: unattended`）。

### 5.2 一个请求的状态

```
            ask_kind
  (none) ───────────────> OPEN ──────────────┐
                            │                 │
   MRTR: PARKED (PendingTable，requestState = gg1.<inst>.<nonce>.<hmac>)
   同步: DIALOG / LEGACY_ELICIT（Human.open 计数）
                            │
    ┌───────────────┬───────┴─────────┬──────────────────┬──────────────────┐
    ▼               ▼                 ▼                  ▼                  ▼
 ANSWERED        LATE             EXPIRED_NO_RETRY     VOID               ANSWER_UNUSED
 (accept/decline (重试在           (过 deadline+5 s     (gate_reset /       (重试到了，但这次调用
  /cancel 映射)   deadline+5 s 后)  仍无重试)            门退出)              没走到问人：冻结 /
                                                                          效果变了 / 不再需要)
```

每个终态都在门日志里留一条 `human_outcome`，带请求 id `req`（requestState 的 nonce；签名本身不进日志）：

| 终态 | `human_outcome` 记录 | 对动作的结果 | 拒绝记录？ | 不再问？ |
|---|---|---|---|---|
| approve：accept + allow | `accepted` | 发一次许可，执行一次 | — | — |
| approve：accept + deny | `denied` | 拒绝 | 是（只会拒绝） | — |
| decline / cancel（无原因） | `declined` / `cancelled` | 拒绝 | 否（no-answer 类） | 是 |
| 客户端给原因 `timed_out` / `expired` / `closed` / `unavailable` | `timeout` / `expired` / `window_closed` / `channel_unavailable` | 拒绝 | 否 | 是 |
| answer：非空文本 | `accepted`（answer 不等于批准） | 只回答问题 | — | — |
| takeover：done=true | `accepted` | `done_by_person`，门不发原动作 | — | — |
| takeover：done=false | `denied` | 拒绝 | 是 | — |
| ask_help：did_it | `accepted`，`choice: did_it` | `done_by_person` | — | — |
| ask_help：hint / try_other | **`accepted`**，`choice: another_way`（v2 修正） | 不执行，提示“换个办法”，带人的提示 | 否 | — |
| LATE | `timeout`（`human_answer.late`） | 拒绝 | 否 | 是 |
| EXPIRED_NO_RETRY | `timeout`，`no_retry: true`（v2 新增） | —（调用已结束） | 否 | — |
| VOID | `void`，`why: reset | gate_exit`（v2 新增）；reset 只作废不撤回，到期再记 `timeout`（`voided: true`） | 旧 requestState 被拒（-32602）；reset 后冻结持续到人回答或超时（v2-sec） | 否 | — |
| ANSWER_UNUSED | 原 outcome，`unused: frozen | effect_changed | not_needed`，`executed: false`（v2 新增） | 不执行；冻结时 fail closed | 否 | — |
| 重复回复（同一 requestState 再来） | `human_answer_rejected` | JSON-RPC -32602 | — | — |
| 迟于门重启的回复 | `human_answer_rejected`（earlier gate process） | -32602 | — | — |
| 桌面对话框：按钮 / 关闭 / 超时 / 助手进程失败 | `accepted|denied` / `window_closed` / `timeout` / `channel_unavailable` | 同上 | denied 才有 | 无回答类才有 |
| 无通道（chat） | `channel_unavailable`（ask_human / handoff / request_human 都记） | 拒绝或 `input_required` | 否 | — |

“不再问”：没得到回答的请求按（kind + 效果键）或（kind + 问题文本）记在 `Inner::unanswered`，同一任务版本内不再弹（`not_asked_again`）。

规则（全部 fail closed）：

- 只有 `action: accept` 且内容为肯定的 approve 表单才批准；answer / takeover / ask_help 永远不批准风险步骤（`gated_inner` 里
  `kind == "approve"` 的判断）。
- MRTR 回答绑定工具 + 参数（`binding_of`）和提问时的效果键；效果变了就不复用，重新问（`asked_again`）。
- 超时、关闭、取消、通道不可用都不是“用户反对”：不写拒绝记录，不进训练标签（`valuetrain` 的 no-answer 列表）。
- `gate_reset`（agent 也能调）只作废待答请求、不撤回：冻结持续到人回答（回答不批准任何东西）或超时（`PendingTable::void_live`，
  v2-sec）；超时扫描 `sweep_expired` 也处理作废请求，到期记 `timeout` 并解除冻结。
- ask_help 的冲突 / 含糊表单（如 `{choice: "hint", done: true}`）是拒绝（`conflicting_form`），只有明确的 hint / try_other 才是 `accepted` + `another_way`。
- 门退出时：MRTR 待答作废并记录；打开着的桌面对话框随门的子进程一起被杀（`run_with_timeout` 把对话框放进自己的进程组并
  `proc::register`，`shutdown` / 信号处理里 `kill_all`）。
- 桌面对话框的默认按钮永远是拒绝的那个（zenity `--default-cancel`、osascript `default button` = 否、GTK
  `set_default_response(deny)`、WinForms `AcceptButton = $no`）：对话框抢焦点时用户正在敲的 Enter 不会变成批准。kdialog 例外（见第 8 节）。
- 桌面对话框里输入的文字走文本通道（`text:` 前缀 / GTK `typed`），不会和 allow / deny / timeout 这些按钮词混淆（`human::dialog_result`）。

### 5.3 计数与负担

`human_asked` 只在请求结束时计一次（MRTR 的停放 + 重试不再算两次）；`note_burden` 记录询问数、重复、等待/主动秒数、接管步骤、
无回答率，进 `gate_report.person_burden` 和日志 `person_burden`。

## 6. 返回给 agent 的上下文

### 6.1 原则

- 只告诉 agent 下一步该做什么（读窗口 / 换目标 / 换办法 / 停下汇报），以及“没执行”这个事实。
- 不给它可以拿来“说服”门的信息：审核模型的原话、模型名、阈值都不回给 agent。
- 不可信文字（应用、网页、终端、命令输出）不能冒充门的消息。

### 6.2 各类文案

| 场景 | 文案要点 | 结构化字段 |
|---|---|---|
| look_light / look_full | 已附新树 / 截图，从中选目标重发，不用自己再读 | `gate.decision`、附观察 |
| reason | 多想一次再发，新步骤照常过门，重想不授权任何东西 | `next_action: rethink`、`reasoning_budget_tokens` |
| deny（规则 / 审核 / 人） | 不要重试，不要换路达到同一效果；给安全替代（`authz::alternative`）；多次拒绝后要求停下汇报 | `gate.decision: deny`、`human_outcome` |
| 无回答 | “没有回答，不是人的拒绝”；同一步本任务不再问 | `human_outcome: timeout …`、`not_asked_again` |
| done_by_person | 人做了，别重复，从当前窗口继续；附重读 | `status: done_by_person` |
| ask_help another_way | “人让你换个办法” + 人的提示 | `human_outcome: accepted`、`human_choice: another_way` |
| stop_loop | 同一步再发没用，换明显不同的办法 | `gate.decision: stop_loop` |
| freeze | 有人在回答，等答完再读窗口 | `gate.tier: freeze` |
| unknown | 不知道是否发生，先读窗口，绝不盲发 | `status: unknown` |

### 6.3 v2 修正（本分支）

- 审核拒绝时，文案只说“the gate's independent reviewer refused it”，审核模型的 `reason` 只进日志（`risk_gate.reviewer`）。
  原来把原话回给 agent，等于给了一个反复改写步骤（或 `gate_task` 里自述的请求，审核在没有启动者任务时能看到它）直到审核同意的预言机。
- `gate_report`（agent 可调）里 `reviewer` 只给 `configured` / `none`，不再给 `api:<协议>:<模型>`。
- `p_direct` 仍在拒绝的结构化结果里：它只影响观察与否，不影响风险路径，暂不隐藏（见第 8 节）。

### 6.4 观察注入

驱动结果（文本项和 `tree_markdown`）、命令的 stdout/stderr、后台任务输出、AT-SPI 读出的终端文字和表格单元格，凡是某行以
`[gate]`、`[human via`、`[client`、`[gui-pi`、`[gui-gate]`、`[run_command]` 开头（忽略前导空白、引号、列表符号，大小写不敏感），
在行首加 `(app text) `（`util::defang` / `defang_result`）。门自己的行不经过这一步。

## 7. 安全不变量

1. agent 不能批准自己的步骤：`gate_ack` / `gate_confirm` 被丢弃；没有 agent 可用的确认码；`requestState` / `inputResponses` 只在
   `params` 层被读，不在 `arguments` 里（`binding_of` 去掉它们）。
2. 只有启动者原话（任务文件）能凭文字授权风险效果；`gate_task` 只是上下文。
3. 风险 / 接管步骤只能以安全动作结束，除非可信来源（启动者任务、审核、人）批准了这一个效果（`safety_decision_ok`）。
4. 每次派发都过 `Gate::tool` 的最终检查：风险目标需要一次性许可，绑定最终参数签名、效果键和任务版本。
5. 拒绝记录只会拒绝；较新任务版本明确点名时才被取代（agent 做不到）。
6. 问人：超时 / 关闭 / 取消 / 通道不可用 / 进程退出 / 迟到回复 / 重复回复 都不是批准。
7. 有人在回答时冻结所有动作类调用；门自己的对话框永远不是目标（`own_dialog_check`）。
8. 门重启后旧 requestState 全部失效（每进程随机 instance + 签名密钥）。
9. 每个停放的问人请求都以一条 `human_outcome` 结束（含 `req`）：回答 / 迟到 / 无重试 / 作废 / 未使用。
10. 审核模型的原话、模型名不进入 agent 上下文；不可信文字不能以门的消息前缀开头。
11. 门知道的密钥在每个结果和每行日志里脱敏（`redact_value`）。
12. 无审核配置时风险步骤拒绝（门不能无审核启动，0.9.3 起）。

## 8. 已知限制

- 威胁模型是“合作但会出错”的 agent：同用户任意代码（`run_command`）可以读门日志（含审核原话、启动者任务）、读写任务文件、
  用 xdotool 之类发合成输入。冻结只拦门自己的动作通道，不拦已在后台运行的命令（`run_command mode=start` 或 `cmd &`）。需要 OS 级隔离。
- `gate_report.log` 仍给出日志路径（运维用）；日志里有审核原话。
- zenity / kdialog 的“取消”和“关窗”退出码相同，记为 `denied`（会写拒绝记录，偏保守）；kdialog `--yesno` 没有默认“否”的选项。
- 一个 MRTR 请求无重试过期后若客户端又迟到重试，会再有一条 `human_outcome`（`timeout`，`human_answer.late`）；按 `req` 去重。
- `EXPIRED_NO_RETRY` 只在下一次工具调用或门退出时被扫到，时间戳是扫描时刻（`waited_s` 给出真实等待）。
- 无重试过期的请求不进 `unanswered`（停放时没有存问人键），同一效果之后可能再问一次。
- 拒绝结果里仍有 `p_direct` 和 `tier`；理论上能帮 agent 探测价值模型，但不影响风险路径。
- `defang` 只按行首判断；行中间的 `[gate]` 不处理，多行标签被驱动合并成一行时也不处理。
- 冻结期间到达的、带着人回答的重试会被拒（fail closed），人的回答作废（记 `unused: frozen`），agent 重发时会再问一次。

- 合并 v2-sec 后：`gate_reset` 只作废、不撤回待答请求（冻结持续到人回答或超时），每个新作废请求记一条 `void`（why=reset），
  到期后扫描再记一条 `timeout`（`voided: true`），因此被 reset 作废的请求有两条 `human_outcome`；人对作废请求的回答只记
  `human_answer_rejected`，不另记 `human_outcome`。
- 冻结前已启动的后台命令（`run_command mode=start`、`cmd &`）在冻结期间仍能发输入（xdotool 等）；门拦不住，需要 OS 级隔离。
- `gate_report` 仍给出日志路径（`gate_report.log`），日志里有审核原话和启动者任务。
- kdialog `--yesno` 无法把默认按钮设成“否”（没有对应选项）；只有 zenity / osascript / GTK / WinForms 默认拒绝。
- 标注规则：同一 scroll 立即重复被记为失败（scroll 35 个“失败”里 23 个是立即重复），很可能是标签错而不是 scroll 失败；
  是否加 scroll 专用重复规则（类似导航键）待定。
- 治疗模型训练器的默认 L2 可能过强：同一划分上未加权拟合 AUC 0.735，放弱 L2 后 0.795；未改默认（会改变所有已训练模型），
  只应在开发任务上调。

## 9. 待在线验证的假设

- 价值模型 v2（`GUI_GATE_VALUE_MODEL=v2`）对任务成功率的影响：离线 AUC 无增益，需按 `online_plan.json`（108 题 × 2 臂 = 216 次运行）
  做成对在线比较；尚未执行。
- look_full / review 的代价（实测 26.4 s / 15.8 s vs 假设 9 s / 3 s）若改进默认代价表，决策会变；需在线验证后再改。
- 冻结前启动的后台命令仍可发输入、`gate_report` 给日志路径、kdialog 无默认拒绝、scroll 重复的标注规则、L2 强度：
  见第 8 节，均未在本分支解决。

- 桌面壳的弹窗在超时 / 关闭时总是带 `_meta[...].reason`；没有原因的 decline 会被记为 `declined`（人拒绝）。
- 客户端（gui-pi）把 `human_outcome: accepted` + `status: blocked` 理解为“人回答了但没执行”，不会提示“人拒绝了”；
  `human_choice: another_way` 对模型足够清楚（gui-pi 目前忽略该字段，只看文本）。
- 真实桌面上 zenity `--default-cancel`、GTK 默认响应、WinForms `AcceptButton` 的实际焦点行为与预期一致（CI 不弹真窗口）。
- 进程组里的对话框在门被 SIGTERM / 客户端关 stdin 时确实消失（Linux 已由测试覆盖；macOS osascript、Windows PowerShell 未实测）。
- `(app text)` 标记不会干扰模型对树的理解（树行以 `- ` 开头，正常不受影响）。
- 审核拒绝只给通用文案后，agent 仍能选对安全替代（替代来自 `authz::alternative`，不依赖审核原话）。
- MRTR 的 `human_timeout + 5 s` 宽限足够覆盖桌面壳的 IPC 往返。

## 10. 测试索引

- Rust（`cargo test`）：`human::tests`（`typed_answers_never_collide_with_button_words`、`dialogs_default_to_the_refusing_button`、
  `expired_requests_are_reported_once_and_still_resolve_late`、`ask_help_another_way_is_an_answer_not_a_refusal`、
  `mrtr_answer_use_is_tracked`、`a_voided_request_keeps_the_freeze_until_its_timeout_is_swept`、
  `conflicting_ask_help_forms_are_not_another_way`、v2-sec 的 `conflicting_or_ambiguous_forms_are_refusals`、
  `nobody_but_the_person_or_the_timeout_ends_a_request` 及原有用例）、`util::defang_tests`、`server::tests`。
- Python（真门 + `tests/fake_driver.py`）：`tests/test_human_protocol.py`（本分支：问人终态记录、作废、未使用、老客户端错误、
  计数、ask_help 换办法、桌面对话框默认按钮 / 输入文字 / 随门退出、审核原话与模型不泄漏、应用与命令文字去冒充），
  `tests/test_sec_regressions.py`（v2-sec 五个回归）、`tests/test_offline_v2.py`（v2-val 离线评估）、
  `tests/test_actions_e2e.py`、`tests/test_remaining.py`、`tests/test_security_e2e.py` 等。
