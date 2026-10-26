# gui-gate 安全模型（v0.10.3 未发布，2026-10-26；0.10.1 起）

## 一句话

gui-gate 防的是**合作但会犯错的 agent**：它想完成用户的任务，但会点错、看错、被界面里的文字误导、把自己的猜测当成用户的意思，
或者在出错后换个工具再试一次。门保证这类 agent 的危险动作（删除、覆盖、移动、发送、购买、改权限……）只有在可信来源批准后才执行，
而且批准只对那一个具体效果有效。

它**不能**阻止一个**恶意模型**：只要模型能以与门相同的系统用户运行任意代码（`run_command`、终端、脚本解释器、从 GUI 终端输入），
它就可以绕开门直接动文件、读门的进程状态、直接连驱动、或伪造任何本地输入。关键词和解析规则只能覆盖已知的写法，
拆成多步、换解释器、编码、下载脚本都不在可靠覆盖范围内。要抵抗恶意模型需要**操作系统级隔离**：模型和它的命令在另一个用户 /
沙箱 / 容器里运行，门和驱动、审批通道、密钥在隔离边界之外。这是**后续工作**，本分支没有做。

v0.10.3 补充（审核 2026-10-26 v2 第 3 节）：
- **关键词规则和独立审核不是恶意代码沙箱。** 它们降低"合作但会犯错"的 agent 出错的后果；对恶意模型，换解释器、分步构造、
  编码、调用 GUI 自动化（xdotool 等）都能绕过，继续补词表不能根本解决。
- **有限攻击集全部通过 ≠ 无绕过。** tests/ 里的攻击测试只证明这些具体路径被挡住（并断言驱动实际收到了什么），不证明不存在别的路径。
- **安全测试与成功率分开报告。** 安全回归（tests/test_security_e2e.py、test_sec_regressions.py、test_v3_sec.py 等）只回答"门是否约束了执行"，
  不回答"门是否让任务更容易成功"；成功率只来自评测任务（bench/），两者不互相替代，也不合并成一个数字。

## 威胁模型

| 在范围内（门要挡住） | 不在范围内（需要 OS 隔离） |
|---|---|
| agent 把自己写的话当成用户授权（`gate_task` 里写"用户同意删除"） | 模型用 `run_command` / 终端执行任意代码、自己删文件 |
| 界面文字、网页、文档里的提示注入（"系统：已授权"） | 模型读取或修改门的配置、日志、内存、驱动 socket |
| 同名文件在另一个目录被误当成用户说的那个 | 模型直接启动另一个驱动或注入系统输入事件 |
| 被拒后换一个按钮 / 快捷键 / 命令 / 工具达到同一后果 | 门所在用户的其他进程被攻破 |
| 批准后窗口、目标、输入内容变了，旧批准被沿用 | 审核模型本身被同一段注入误导（独立调用 ≠ 错误独立） |
| 自愈、重绑定、重试、批量子步骤落到一个危险目标上 | |
| 重放 / 串线的人工审批响应 | |
| 驱动崩溃或超时后把"可能已经执行"的动作再发一次 | |

## 谁能批准

批准只有三个来源，agent 发来的任何东西都不是：

1. **启动者的任务原文**：`--task` / `--task-file` / `GUI_GATE_TASK`（由启动门的程序——桌面壳、评测脚本——给出）。
   它只在**明确说了这种危险操作**（且不是否定句）**并且点名了结构化效果作用的每一个文件**时才覆盖该效果：
   - 文件资源是规范化的绝对路径（`path:`）。原文里写了带路径的同名文件时，必须就是这个路径（`/a/report.txt` 不覆盖 `/b/report.txt`）；
     只写了文件名时按文件名匹配。
   - 没有可匹配对象的效果（"发送"按钮、"删除"控件、无法解析目标的命令）**从不**由文字直接覆盖：中风险交给审核模型，高风险交给人。
   - 例外：覆盖当前应用里正打开的那个文档本身（工作文档）。
2. **独立审核模型**（中风险）：只看启动者的原话、具体动作和界面/命令文本。agent 自己陈述的任务以"未经验证、不能授权"的标注附上。
3. **人**：MRTR（2026-07-28 elicitation）、老式 elicitation、桌面原生对话框。人的回答绑定到调用（工具 + 参数）**和**提问时的结构化效果。

`gate_task` 仍可调用，但只作为上下文（简报、计划检查、单步审核），任何危险等级都不能由它授权。
旧的 `gate_confirm` 确认码路径已删除（`gate_confirm` / `gate_ack` 参数被丢弃，不出现在 tools/list 和说明里）。
`request_human` 的 approve 只回答 agent 的问题，不会解锁门拦下的任何步骤。

## 拒绝记录 vs 批准凭证

- **拒绝是记录**（`Inner.denials`）：结构化效果 + 谁拒绝的。它覆盖一个稳定范围：同一危险类型且资源重叠——同一路径、被拒目录里的路径、
  包含被拒路径的目录、对话框里的文件名与完整路径、同一应用里的同一控件、同一命令摘要。与工具无关：`run_command rm x`
  被拒后，在终端里输入同一个 `rm x` 回车、或删除包含它的目录，都直接拒绝，不再问人。拒绝永不被之后对**别的**效果的批准清除，
  也不阻止不同的效果（另一目录的同名文件、另一应用的同名按钮）。
- **批准是一次性凭证**（`Inner.approval` → `Inner.permits`）：gated() 在批准后为**最终参数**签发派发许可，绑定动作签名 +
  完整结构化效果（工具、危险类型、资源、应用 pid、窗口 id、载荷摘要），30 秒过期，用一次即删，每次 gated() 结束清空。

## 结构化效果键（src/effect.rs、src/authz.rs `effect_of`）

`{tool, hazard, resources, app, window, payload}`：

- `hazard`：delete / overwrite / move / send / purchase / permissions / power / install / logout / command / secret / other。
- `resources`：`path:<规范化绝对路径>`（sidefx/command 的磁盘检查给出的目标：cp/mv 覆盖、mv 移走、rm、`>` 重定向、soffice 转换、
  chmod -R、find 根目录、解释器代码写入的已有文件、保存对话框的目标），否则 `name:<对话框里的文件名>`，否则 `elem:<role>|<label>`，
  否则 `cmd:<命令摘要>`。一条命令里多个危险部分的目标取并集。
- `payload`：输入文字、按键、命令行、终端里 Enter 会提交的那一行、cwd 的摘要。

## 哪里强制了什么

| 位置 | 强制的内容 |
|---|---|
| `Gate::tool`（唯一派发点） | 每个 GUI 动作（gated 直接执行、焦点/遮挡/后台重发、heal_occlusion 自己的点击和按键、批量子步骤、终端清行）对**最终参数**重建 ActionCtx；危险或需人接管的目标没有匹配的一次性许可就拒绝，什么也不发。`run_command` 在 gated 里过同一个检查。 |
| `Gate::grant` | 只有当最终参数的效果与被批准的效果**完全相同**才签发许可；重绑定/重试落到别的目标、窗口、载荷上得不到许可。 |
| `Gate::tool_raw` | 动作工具在驱动退出、超时、返回无法解析时**不重发**，重启驱动，返回 `status: "unknown"`；同一签名下次发来时门先读窗口（look_light），之后重新决策。 |
| `gated()` | 安全规则先于基线策略；未知结果规则；风险路径（拒绝记录 → 授权 → 审核 → 人）；人的回答结果（human_outcome）。 |
| `risk_gate` | 拒绝记录覆盖检查；只用启动者任务授权；审核模型。 |
| `human.rs` | MRTR requestState 一次性、绑定工具+参数、过期即超时、伪造/重放/串线回 -32602；回答绑定效果和任务版本，变了重新问；冲突回答按拒绝；待答请求只能由人或超时结束；记录结束原因。 |
| `proc.rs scrub_env` | 子进程（驱动、命令、角色探测）只拿白名单环境变量，不继承密钥。 |
| `gate_config` | 只有可信启动者用 `--allow-config` / `GUI_GATE_ALLOW_CONFIG=1` 才可用。 |

## 人的请求为什么结束

`structuredContent.human_outcome` 和日志事件 `human_outcome`：`accepted`、`denied`（人选了否）、`declined` / `cancelled`
（客户端表单被拒/取消）、`timeout`、`window_closed`（桌面对话框被关掉）、`channel_unavailable`（没有能问人的通道）、
`unattended`（`--human deny`）、`no_response`。除 accepted 外一律按拒绝处理；超时不等于用户反对，只是这一次没执行。

## 结果状态

`structuredContent.status`：`executed`、`blocked`（门没执行）、`partial`（批量执行了一部分）、`done`（非动作工具）、
`unknown`（可能执行了也可能没有：先读窗口，不要盲目重发）、`input_required`（需要人：批量里要单独发的步骤、无法联系人时的接管、
`request_human` 没有通道）。MRTR 的 `resultType: "input_required"` 是协议层的，见 docs/mcp-2026-07-28.md。

## 0.10.1：哪些工具算“动作”（按分类拒绝）

- **只读工具是白名单**（`src/features.rs` `READ_ONLY_TOOLS`）：`get_window_state`、`get_desktop_state`、`get_browser_state`、
  `list_windows`、`list_apps`、`screenshot`、`zoom`、`verify_state`、`get_cursor_position`、`get_screen_size`、`list_displays`、
  `list_screens`、`check_permissions`、`get_permissions`、`get_config`、`get_version`、`get_accessibility_tree`、`find_elements`、
  `get_element`、`get_focused_element`、`get_clipboard_text_length`。可信启动者可用 `GUI_GATE_READONLY_TOOLS` 追加（agent 改不了环境）。
- **其余一切都是动作**：门已建模的 GUI 工具（click、type_text……）、`launch_app`、`bring_to_front`、以及新版驱动将来新增的任何工具。
  没有 GUI 目标模型的“透传动作”也**从不原样转发**：都经过 `Gate::tool` 里同一个最终派发检查（一次性许可、输入冻结、自己的对话框）。
- **`launch_app` 的风险**（`launch_risk`）：交给任何应用的 URL（浏览器、xdg-open、mailto……）为中风险；终端 / shell / 解释器带命令
  （`-e`、`-x`、`--`、`-c`、`/c`……之后的部分，否则全部参数）按 `run_command` 同样的命令风险分类，分类不出来也至少中风险；
  文件管理器打开某个路径为中风险并以该路径为资源。有风险的透传动作走与所有动作相同的风险路径（拒绝记录 → 启动者任务 → 审核 → 人）。

## 0.10.1：人在回答时冻结输入，保护门自己的对话框

- 只要有一个问人的请求还开着（桌面对话框打开中，或 MRTR 待答记录还没过 `deadline + 5 s`），**所有动作类调用**（驱动动作、
  `run_actions`、透传动作）在入口和最终派发点都被拒绝（`tier: "freeze"`，日志 `frozen`），什么也不发。agent 不能在人回答期间操作界面，
  更不能替人点批准按钮。观察类工具照常可用。
- 门自己弹出的对话框进程（zenity / kdialog / osascript / GTK python / PowerShell）的 pid 记录在案；目标是这些 pid 或标题是门对话框的
  窗口的动作，在任何决策之前就被拒绝（`tier: "own_dialog"`，日志 `own_dialog_refused`）。只有人能回答门的对话框。

## 0.10.1：只写文件名的任务

启动者原文只写了文件名（没有路径）时，它只覆盖**工作目录里**的那个文件：`run_command` 的 cwd，或终端当前目录。另一个目录里的同名文件
不由文字授权（可能是用户没指的另一个文件），交给审核模型 / 人。同一个效果里出现两个同名不同目录的文件时，只写文件名的原文不授权任何一个。
写了路径的原文仍必须是那个规范化路径。

## 0.10.1：可信任务通道 `--task-file`（桌面客户端格式）

桌面客户端把**只有人说的话**写进任务文件（0600），门在每次风险判断前检查文件是否变了并重读：

```
gui-pi-task/1 version=N
--- user 1 <UTC ISO 时间> initial request
用户的第一条原话……
--- user 2 <UTC ISO 时间> later message
用户之后补充的原话……
```

- 第一行是格式头和客户端版本号 N；每条用户消息一个 `--- user <i> <UTC> initial request|later message` 段，段内是原文。
  门把各段原文按顺序拼起来作为授权用的启动者原话。没有格式头的文件整个当作任务原文（门按内容哈希给版本）。
- 每个新版本记一条日志 `task_version`（版本号、客户端版本、内容哈希、字数，不记原文）。
- **批准绑定任务版本**：一次性许可记下签发时的任务版本，版本变了就失效；超时未答的“不再问”也只在同一版本内有效。
- 拒绝记录仍然只会拒绝；但如果用户在**更新的版本**里明确点名了这个效果（授权规则照常适用），旧版本下的拒绝被取代
  （日志 `denial_superseded`）。agent 自己的话（`gate_task`）永远做不到这一点。

## 0.10.1：MRTR requestState 按进程签名

**它证明什么、不证明什么（v0.10.3 更正）**：HMAC 只证明这个 `requestState` 是**本门进程签发的、没被改过、绑定这一个调用**；
它**不证明**随重试带回来的 `inputResponses`（“allow”）来自真人。门看到的“人回答”是经手这条状态的客户端扩展（gui-pi）交回来的：
任何能拿到合法 `requestState` 并调用 `tools/call` 的组件都能填一个 allow。所以**经手合法状态和答案的扩展 / 桌面壳属于可信计算基**，
不是“半可信、无法代答”。agent（模型）本身拿不到 `params` 层（只能写 `arguments`，门从 `arguments` 里丢掉这些键），这是边界所在；
扩展若被攻破或写错，可以代答。

`requestState` 的形式是 `gg1.<instance>.<nonce>.<HMAC>`：instance 和签名密钥每次门进程启动时随机生成，HMAC-SHA256 覆盖
instance、nonce 和绑定（工具 + 参数）。门重启后旧进程签发的 requestState 一律被拒（-32602，“the gate restarted; every earlier approval is void”）；
格式不对、签名不对、未知或已用过、绑定的调用不同，都回 -32602。仍是一次性的，仍按 `human_timeout + 5 s` 判超时。

## 0.10.1：严格的人回答类型

四种请求各有唯一的结果形状，互不转换：

| 类型 | 表单 | 只有这样才算“是” | 意义 |
|---|---|---|---|
| approve | `{decision: allow\|deny}`（兼容老 `{allow: bool}`） | `accept` + `allow` | 只批准门确认的那一个效果一次 |
| answer（澄清） | `{answer}`，有 2–4 个选项时另加 `choice` | `accept` + 非空回答 | 回答**不是**批准 |
| takeover | `{done: bool}` | `accept` + `done: true` | 人自己做了这一步 → 结果 `status: "done_by_person"`，门**不会**再发原动作，先重读窗口 |
| ask_help（求助） | `{choice: did_it\|hint\|try_other, note}` | `did_it` = 人代做（同样 `done_by_person`）；hint / try_other 只带回提示 | 不批准任何东西 |

- 没有回答的结束（`timeout`、`cancelled`、`declined`、`window_closed`、`no_response`、`channel_unavailable`）按拒绝处理本次动作，
  日志标 `no_answer`，**不当作用户反对**，也不进价值模型训练标签。
- **超时后不再问**：同一个效果（或同一个 `request_human` 问题）在同一任务版本里没得到回答，就不再弹第二次（`human_outcome: "not_asked_again"`），
  告诉 agent 别猜、停下汇报。用户在任务文件里补了新消息（版本变了）后才可能再问。
- 安全上必须问的步骤从不参加价值/成本比较；可选的澄清和求助才算问人成本。问人负担（询问数、重复比例、等待/主动秒数、接管步骤、
  无回答率、不再问次数）进 `gate_report` 的 `person_burden`、日志 `person_burden` 事件和 `gui-gate report 日志`。

## 0.10.1：`gate_reset`

客户端停止或重启 agent 时调用（客户端专用工具，见 docs/mcp-2026-07-28.md）。之后：所有一次性许可、未完成的批准、待答的问人请求
（requestState）、缓存的观察和快照 id 全部作废——旧元素 token 不再解析、旧 requestState 被拒、重置前批准的东西都不延续。
拒绝记录和启动者任务保留（拒绝只会拒绝）。

## 0.10.1：密钥脱敏

门知道的密钥值（环境里名字像 `*_KEY`、`*TOKEN*`、`*_SECRET`、`*PASSWORD*` 的变量、密钥文件、`reviewer.env` 设置）以及通用密钥形状
（`sk-…`、`ghp_…`、`xox?-…`、`AKIA…`、`Bearer …`、`KEY=…` / `password: …`、JWT）在**每个工具结果**（图片数据除外）和**每行日志**里替换成
`[redacted]`，不管它走了什么路（命令打印了环境、解释器读了密钥文件、错误信息）。子进程环境白名单（`scrub_env`）照旧。脱敏是兜底，
不是隔离：同用户任意代码仍可直接读文件（见开头）。

## 测试

- Rust：`effect::tests`（同名不同目录、跨工具同一效果、按应用区分控件）、`authz::tests`（只认点名的文件、没有对象的效果不靠文字覆盖、否定句）、
  `human::tests`（结束原因、效果变化后不复用回答）、`server::tests::final_dispatch_check_needs_a_one_shot_permit`（真门 + 假驱动：无许可、
  别的窗口的批准、用过的、过期的许可都被拒，驱动只收到一次）。
- Python（真门二进制 + tests/fake_driver.py）：`tests/test_security_e2e.py` 的绕过测试，`tests/test_remaining.py`（0.10.1：透传动作、launch_app、冻结、自己的对话框、gate_reset、签名 requestState、任务文件版本、严格回答类型、不再问、脱敏），`tests/test_actions_e2e.py`、`tests/test_gate.py`、
  `tests/test_v04.py`、`tests/test_commands.py` 中的边界测试。

## v2-sec（2026-10-26）：五个回归修复

- **冲突回答一律拒绝**：approve / takeover / ask_help 表单里同时出现“是”和“否”（如 `{allow: true, decision: "deny"}`）、
  `allow` 不是布尔值、`decision` 不是精确的 `allow`/`deny`，都按拒绝处理（`human::conflicting_form`）；原生对话框输出不是它自己的按钮也按拒绝（`human::map_desktop`）。
- **批准绑定任务版本**：MRTR 回答绑定提问时的任务版本和 reset 次数（`task_binding`），任务文件出了新版本后重新问人；
  同步通道（对话框 / 老式 elicitation）回答期间任务变了，回答不批准任何东西（`human_outcome: "task_changed"`）；许可记的是批准时的版本（`approval_tv`）。
- **读不写盘**：只读工具带写入目标参数（`screenshot_out_file`、`output_path`、`save_to`、值是文件路径的参数……），以及任何工具带“截图存文件”参数，
  在调用入口和唯一派发点都被拒绝，什么也不发（`features::write_args`，`tier: "write_args"`）。
- **待答问题只能由人或超时结束**：`gate_reset`（agent 也能调）只作废待答请求、不撤回，输入冻结到人回答或超时；
  绑定别的调用或不带回答的重试会烧掉这次批准，但同样不结束冻结（`PendingTable::resolve` / `void_all`）。
- **未建模动作结果 unknown 后不盲目重发**：透传动作（launch_app、set_clipboard、新驱动工具）结果 unknown 后，同一工具 + 参数（忽略键顺序和投递方式）
  再次派发先被拒一次并附当前窗口列表，之后的有意重发重新决策（`Gate::pass_action`、`unmodelled_key`）。

## v2（分支 v2-hum）：问人收尾、对话框默认按钮、agent 上下文

- 桌面对话框默认按钮是拒绝的那个（zenity `--default-cancel`、osascript `default button` = 否、GTK 默认响应 = 拒绝、
  WinForms `AcceptButton = $no`）：对话框抢焦点时的 Enter 不会批准。输入的文字走文本通道（`text:` / `typed`），不会被当成
  allow / timeout 等按钮词（`human::dialog_result`）；助手进程失败记 `channel_unavailable`，不是“人拒绝”。
- 对话框进程放在自己的进程组并登记到 `proc`：门退出（客户端关 stdin、信号）时随其它子进程一起被杀，待答 MRTR 请求作废并记录。
- 审核模型拒绝的原话不回给 agent（只进日志 `risk_gate.reviewer`），`gate_report.reviewer` 只给 `configured|none`。
- 应用 / 网页 / 终端 / 命令输出里以 `[gate]`、`[human via`、`[client`、`[gui-pi`、`[gui-gate]`、`[run_command]` 开头的行加
  `(app text) ` 前缀（`util::defang`），不能冒充门的消息。
- 与 v2-sec 合并后的语义：`gate_reset` 对每个新作废的请求记一条 `human_outcome`（`void`，why=reset），但不撤回、不解冻；
  人回答（不批准任何东西）或超时才结束冻结，超时由扫描记 `timeout`（`no_retry`、`voided: true`）。门退出对未作废的请求记 `void`（why=gate_exit）。
  冲突的 ask_help 表单（如 `{choice: "hint", done: true}`）仍是拒绝，不算“换个办法”。
- 详见 docs/gate-design-v2.md。

## v0.10.3（分支 v3-sec）：派发、unknown、审批前提、审核失败

审核 2026-10-26 v2 第 3、4 节；清单见 docs/review-v3-checklist-sec.md。每条都有 tests/test_v3_sec.py 回归（真门二进制 + fake_driver，
按 `FAKE_ACTION_LOG` 断言驱动**实际收到**的动作次数和内容）。

- **每一次实际发送都过同一个检查入口。** `Gate::tool` 是唯一的发送循环：首次发送、后台→前台回退、焦点修复、遮挡修复、批量内部步骤，
  每次真正发给驱动之前都走 `send_checks`（冻结、门自己的对话框、最终派发检查 + 审批前提重读），日志每次一条 `send_check`。
  只有驱动明确说“未送达”（`not_delivered`：后台不可用、前台不可用 *no input was sent*、指针点被别的控件占有、无无焦点动作、
  令牌过期被拒）才自动重试；重试前把刚消耗的一次性许可退回，再完整检查一遍（目标、效果键、任务版本、前提都按当时重新计算）。
  修前：后台回退直接 `tool_raw`，第二次发送不经检查（测试：两次发送之间用户任务换了版本，修前前台重发照样送达）。
- **unknown 是账本，不是一次性提示。** 驱动死亡 / 超时 / 回答无法解析时，动作进 `unknown_sigs`（`ledger::UnknownEntry`：原因、时间、
  窗口、是否非幂等）。只有之后对同一窗口**成功**读取一次才算“看过”；读失败不清记录、继续拒绝。可幂等的步骤看过后才解锁；
  **非幂等效果**（有风险标签或交接目标：发送、删除、支付、提交……）看过也不解锁：门拿不到“确定没执行”的证据、驱动也没有幂等键，
  所以唯一的解锁是真人核实（有人：问“它可能已经发生，确认没发生才再发”，批准才发；无人：拒绝并让 agent 报告“可能已完成”）。
  记录跨 `gate_reset` 保留。透传动作同理（读窗口列表成功一次后，可幂等的才重新决策）。
- **审批绑定业务前提。** 批准（启动者任务、审核模型、人）时记下 `ledger::Premise`：目标窗口最近一次读取里带文字/值的行
  （收件人、金额、正文、表单字段……）的 SHA-256、该读取的快照 ID（对象版本）、效果涉及文件的 (mtime, size)。问人表单的
  `_meta.premise` 给出这些行；答案绑定里包含前提摘要（行变了 = 另一个问题，重新问）。派发前（最终检查）重读目标窗口和文件：
  摘要不同 → 许可和审批作废，返回 `tier: premise`（`premise_changed` 写明变了什么），重发时重新决策 / 问人；令牌和索引按 role+label
  重新绑定到新读取。**读不到关键状态**（没有目标窗口的树、没有文件）时如实标 `premise: {readable: false}`（“不可验证”）：
  表单写明，许可有效期 10 s（可验证的 30 s），并且有人在场时审核模型的批准不够、改问人（无人时审核模型照旧决定，日志记 `permits_unverifiable`）。
- **拒绝键与批准键继续分开。** 批准：一次性、短时、效果键所有字段相等 + 前提相等。拒绝：按资源范围覆盖。basename 回退（对话框只给文件名时
  `name:x.pdf` 覆盖任何同名路径）和“同应用同标签”（同一应用里的同一个 “Send” 视为同一效果）是**保守近似**：只用于拒绝覆盖，
  多拒不多放；批准从不用它们。
- **效果摘要改为 SHA-256**（`effect::digest`，门内纯 Rust 实现，NIST 向量测试）。v0.10.2 及以前日志里的 16 位 FNV-1a 摘要由
  `EffectKey::from_json` 识别为 legacy，只用于读日志 / 统计，永远不会与新摘要相等（旧记录不可能批准任何东西）。
- **审核超时 / 出错不退回 direct。** 价值模型选了 review、审核模型不答：有人 → 问人（批准才执行），无人 → 拒绝（`review_unavailable`），
  不再“选剩下分数最好的选项”。规则路径（有人 ask_help、无人 stop_loop）和风险路径（本来就 fail closed）不变。
  MRTR 重试带回的“不批准 / 没回答”对同一调用直接生效：重试时即使重新决策会走另一条路（如令牌重绑后 direct），也不执行。
- **接管之后**：人做完接管 / 求助“我已做”后，门清空所有许可和审批，记下 `takeover_at`（此前的读取、令牌、索引、坐标不再被直接使用，
  先读再发），重读目标窗口、窗口列表（谁在前台）和人操作期间改过的文件（`structuredContent.reread`）；同一步骤重发进 unknown 账本
  （可幂等：先读一次；非幂等：真人核实）。门从不重放原动作。

## 已知限制

- 只匹配门能解析的效果：任意脚本、编码过的命令、多步拼出来的效果、GUI 里没有文件路径的删除，靠审核模型和人，不靠解析。
- 对话框只给文件名时，资源是 `name:`，拒绝记录按文件名保守覆盖；批准仍精确绑定（含前提）。
- 动作签名对 token 目标不含窗口，但许可同时绑定效果键（含窗口），两者都要相等。
- 前提只覆盖门**读得到**的东西：目标窗口的无障碍树（行的文字 / 值）和效果点名的文件。树里不暴露的内容（画布、图片里的文字、
  另一个窗口里的附件）、文件以外的远端状态（服务器上的草稿）都不在前提里；这时前提要么仍“可读”但不完整，要么标“不可验证”。
- 前提重读多一次树读取（每个已批准的风险发送一次），会让此前的令牌过期；门在同一步里重新绑定。
- unknown 账本的非幂等判定沿用风险标签和交接目标；没有风险标签的非幂等操作（例如某个切换开关）读过窗口即解锁。
- 恶意模型、同用户任意代码执行：见开头，需要 OS 级隔离（单独用户/沙箱，门与驱动、审批、密钥在边界外），后续工作。
