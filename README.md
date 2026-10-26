# gui-gate

> **v0.10.2 是 gui-gate 作为独立 MCP 的最后一版。** 之后的开发转到
> [cszf1/gui-pi-desktop](https://github.com/cszf1/gui-pi-desktop)（Pi 桌面版 computer-use agent），门的代码随桌面版演进；
> 本仓库保留，不再计划发布独立版本。
> **v0.10.2 is the last release of gui-gate as a standalone MCP server; development continues in cszf1/gui-pi-desktop.**
>
> OSWorld 免责声明：本仓库里的 OSWorld / OSWorld-Verified 数字都来自**基于 OSWorld 任务与评测器的自定义环境**
> （原生 Debian/XFCE、无 VM、Chromium、应用版本与预算不同、--human deny），不是 OSWorld 官方成绩，不能与排行榜比较
> （custom environment based on OSWorld tasks and evaluators; not an official OSWorld-Verified score, not comparable to
> the leaderboard）。v0.9.9 运行的失败归类见 docs/failure-analysis-v099.md。

**一个完整的 MCP：门控 + Cua Driver + 批量执行，装一次就能用。** 任何支持 MCP 的 Agent（Claude Code、Codex、Cursor、Pi……）都能接。
门控本身是一个 Rust 程序（约 4 MB，无运行时依赖）；Cua Driver 用它官方的安装器装最新版，一条命令全部装好。

```bash
# macOS / Linux
curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash -s -- --client claude
# Windows（PowerShell）
$env:GUI_GATE_CLIENT="claude"; irm https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1 | iex
```
`--client` 可选（claude / codex / cursor / pi），不写就只装好并自检，之后再 `gui-gate setup --client ...`。

**必须配置审核模型（v0.9.3 起）**：门里的审核模型可以是任何大模型（协议 chat / responses / anthropic / gemini），
也可以是决策模型（协议 systemone）。没配好时 MCP 服务直接拒绝启动（stderr 说明怎么配，退出码非 0），
不存在「不带审核运行」的方式（`GUI_GATE_REVIEWER=off` 已删除，设了也会拒绝启动）。
- 安装时：有终端就交互式询问 URL、模型名、密钥（密钥存到 `~/.config/gui-gate/reviewer.key`，权限 600），协议自动猜；
  无人值守用参数或环境变量：
  `bash -s -- --reviewer-url URL --reviewer-model NAME --reviewer-key-file PATH [--reviewer-protocol P]`，
  或 `GUI_GATE_REVIEWER_URL / _MODEL / _KEY_FILE / _KEY / _PROTOCOL`（Windows 只能用环境变量），`--no-prompt` 不询问。
  仍没配的话安装照样完成，但会明确提示「门在配好审核模型前不会启动」；配好了会顺手跑一次 `gui-gate check-reviewer`。
- 之后配置：写进 `~/.config/gui-gate/reviewer.env`（`KEY=VALUE` 每行一项，门启动时自动读；环境变量优先），
  或写在 MCP 客户端这条服务的环境变量里：
  ```
  GUI_GATE_REVIEWER_URL=https://api.deepseek.com/v1
  GUI_GATE_REVIEWER_MODEL=deepseek-chat
  GUI_GATE_REVIEWER_KEY_FILE=/home/you/.config/gui-gate/reviewer.key
  ```
  然后 `gui-gate check-reviewer`（只查配置不调 API：`--config-only`）。`gui-gate doctor` 也会报告审核模型是否配好。

两种用法同时提供：**单步**（`click`、`type_text`……每次一个动作）和 **批量**（`run_actions`，一次一串步骤，
支持 find / for_each / if / read，功能对标 Pi codemode，但不需要 JS 引擎）。每个动作不管哪种方式都过门控。
`--mode single` 只给单步。Cua Driver 用的是官方预编译版本（macOS、Linux x86_64/arm64、Windows），测试版本 0.38.0（随 gui-gate 发布包附带）。

```
Agent (Pi 等) ──MCP stdio──▶ gui-gate ──▶ cua-driver（手脚：AT-SPI / UIA / AX + 输入）
                              │
                              └─ 每个动作执行前：值不值得先看一眼 / 多想一下 / 等一等？
```

Cua Driver 的 66 个工具原样暴露。只有白名单里的只读工具（`get_window_state`、`list_windows`、`screenshot` 等）直接放行并被门记录；
其余一切（`click`、`type_text`、`launch_app`、`bring_to_front`、新版驱动新增的任何工具……）都算动作，先过门和唯一的最终派发检查（v0.10.1，见 docs/security-model.md）。

## MCP 协议版本

gui-gate 同时支持两种 MCP 客户端：老式的（先发 `initialize`，协议 2025-06-18 等）和 2026-07-28 新式的（不发 `initialize`，每个请求在 `params._meta` 里带 `io.modelcontextprotocol/protocolVersion` 和 `clientCapabilities`）。
gui-gate 启动驱动后自己先和驱动做一次老式 `initialize`（驱动重启后也会重做），之后发给驱动的请求一律走老式；客户端的 `initialize` 和 `server/discover` 由 gui-gate 自己回答（带门的说明和 gui-gate 的 serverInfo）。新式请求的结果都带 `resultType: "complete"` 和 `_meta` 里的 gui-gate serverInfo；不支持的协议版本返回 -32022。详见 docs/mcp-2026-07-28.md。

## v0.10.x 要点（完整列表见 [CHANGELOG.md](CHANGELOG.md)）

- **唯一的最终派发检查**：门发出的每个动作（含自愈、重绑定、批量子步骤、透传的 `launch_app` 等）都按最终参数重建效果，
  没有绑定该效果的一次性许可就不发。只读工具是白名单，其余都是动作。agent 不能批准自己（`gate_confirm` 已删除，`gate_task` 只作上下文）。
- **可信任务通道**：`--task-file` 接受桌面客户端的 `gui-pi-task/1 version=N` 格式（只有人说的话，按版本），批准绑定任务版本。
- **问人**：四种严格结果类型（approve / answer / takeover / ask_help），人回答期间冻结所有输入，门自己的对话框永远不是目标；
  人代做的步骤返回 `status: "done_by_person"`、从不重发；没回答的问题同一任务版本内不再问；问人负担进 `gate_report` 和 `gui-gate report 日志`。
- **客户端专用工具**：`gate_next_step`（下一轮前要不要看 / 多想，只是建议）、`gate_reset`（停止/重启时作废许可、待答请求和观察）；
  客户端可在 `_meta["gui_pi/usage"]` 里报真实 token 用量。MRTR `requestState` 按门进程签名，重启即失效。
- **价值模型**：`extra_reason` = 让同一模型重想一次（1024 token 提示，不授权）；处理交互模型可加 IPW（报告 ESS），`train --report`
  比较交互 / 每选项单独 / 提升树桩三个变体；默认仍是旧先验。
- **planner / search / compact 默认关闭**；回复和日志里的已知密钥一律 `[redacted]`。

## 门的三层判断（v2，详见 docs/decision-v2.md）

| 层 | 做什么 | 代价 |
|---|---|---|
| 0 硬规则 | 高风险（删除/发送/支付…）要用户确认；旧快照的元素 token 自动修复；同一动作刚失败不许盲目重发；新弹窗/新窗口出现要先看；像素点击但窗口变了要截图 | 0 |
| 1 预测模型 | 逻辑回归算 P(直接执行成功)，31 个运行时特征（含「本次会话已失败几次、是否连续失败、上一步有没有反应、点的是不是菜单」）；权重用 2934 个 OSWorld 实测动作训练，留出会话 AUC ≈ 0.76（手写权重 ≈ 0.53） | <1 ms |
| 2 期望代价公式 | `E[代价] = λ·选项耗时(秒) + (1 − 用该选项后的成功率) × 失败代价`；成功率用同一个模型对「选项做完后的状态」重新打分；失败代价 ≈ 19 s × 难撤销程度 × (1+连续失败)^1（0.9.5 起，原为 1.5；`GUI_GATE_FAIL_EXP`） | <1 ms |

### 价值模型（0.9.11 起；详见 docs/value-model.md）

第 2 层的 P_o 现在来自两个**有名字的**价值模型之一，每个决策的日志行 `value` 字段写明用的是哪个：

* `treatment`（处理交互模型）：`P(成功 | 特征, 实际采取的选项)`，逻辑回归 + 选项 one-hot × 特征交互，L2；
  用 `gui-gate train --model treatment` 从日志里**实际采取的选项**及其结果训练。增量 Δ_o = q_o − q_direct
  可以为零或为负（不再截断到 ≥ P(direct)），每个选项有自助法区间；区间跨零且是低风险步骤时取更便宜的选项。
  **只有加载了它的训练权重（`GUI_GATE_VALUE_WEIGHTS`）时才是默认。**
* `legacy`（旧先验，`GUI_GATE_VALUE_MODEL=legacy`）：0.9.x 的做法——把特征手工「重置」后再用 P(direct) 打分，
  加上**手写**的残差收益，强制 P_o ≥ P(direct)。**这些收益是手设的，不是估计出来的。** 目前没有处理交互权重，
  所以现在默认仍是它。

选项代价优先用日志实测（`gui-gate costs 日志 -o costs.json` → `GUI_GATE_COSTS`，含墙钟秒、token 估计、人的秒数），
样本不足时才回退到 7 s / 9 s / 45 s 先验；`check` 按信息价值计算（处理交互模型下默认，`GUI_GATE_CHECK_MODEL`）。
公平对照的固定策略：`GUI_GATE_VALUE_POLICY=direct|observe_light|observe_full|extra_reason|threshold|heuristic|random|learned`，
只替换价值选择，安全规则、硬规则、自动观察、自愈等工程底座不变（bench：`vp_*` 条件 + `bench/fair_compare.py`）。
在拿到真实反事实分支数据（`bench/counterfactual/`）之前，处理交互模型只是观察性估计（受记录策略混杂），不能当作因果增量。

选项：`direct` 直接执行 · `check` 门自己探测（0.2 s）· `heal` 修复过期 token · `wait` 等界面稳定 ·
`look_light` / `look_full` 门自己重读界面（需要时带截图）并随拒绝一起返回，Agent 直接重新瞄准 ·
`review` 门里唯一的大模型——独立审核模型复核拿不准的目标 · `risk_review` 审核模型裁决中等风险 ·
`ask_human` 请用户确认风险动作 · `ask_help` 连续失败卡住时请人代做或指点（有人可找时才会出现）。
**正在干活的主模型（如 Pi 里的）从不被要求确认自己的动作**（v0.8 起没有 gate_ack）。

门拦下时返回以 `[gate]` 开头的一句话，告诉模型具体怎么做，**不会执行任何动作**。动作报错时也会顺带返回最新界面树，
省掉模型再读一次。特征全部来自运行时，不使用任何生成器真值。`gui-gate replay 日志目录` 可以用旧日志回放评估当前判断。

## 截图格式（v0.9.4）

门交给 Agent 的每一张截图（`get_window_state` / `screenshot` / 动作后的自动观察 / `run_actions` 末尾读屏……
cua-driver 结果里所有 `image` 内容）都从 PNG **重新编码成原尺寸 JPEG 质量 60**，`mimeType` 改为 `image/jpeg`。
**不缩小**，所以模型从图上读出的 x/y 坐标照样有效。所有策略（direct / full / light / gate）都一样，基准各条件可比。

依据是实验 `results/screenshot-format-2026-10-21`（deepseek-v4.1-flash，15 个 OSWorld 任务的门截图，980 次调用）：
JPEG q60 原尺寸的准确率在我们的测试里与 PNG **没有显著差异**（配对 18 好 / 17 差，p = 1.0；no significant difference in our tests），但这**不证明与 PNG 等效**（not proven equivalent to PNG）；冷启动首 token 7.0 s 对 21.4 s（3 张图 17.2 s 对 64.8 s），约 3 倍的首 token 加速**只对所测的模型（deepseek-v4.1-flash）和 API 成立**，换模型或接口要重新测；
图片 93 KB 对 326 KB；半尺寸准确率明显下降（25% 对 43%），WebP q80 准确率偏低（38%），都没采用。

| 环境变量 | 默认 | 说明 |
|---|---|---|
| `GUI_GATE_SCREENSHOT_FORMAT` | `jpeg` | `png` = 原样转发驱动的 PNG |
| `GUI_GATE_SCREENSHOT_QUALITY` | `60` | JPEG 质量，限制在 30–95 |

解不开的图（不是 PNG、数据损坏）原样透传。编码用纯 Rust（`image` crate，只开 png+jpeg），1396×823 截图约 20 ms。
日志里每个带图的结果记一行 `{"ev": "img", "format", "images", "converted", "bytes_in", "bytes_out", "ms"}`，
`gate_report` 里有累计字节数和编码耗时。

## v0.9.7 新功能（opt-0.9.7 分支，默认全部关闭；详见 docs/sota-absorb-2026-10-22.md）

吸收 OSWorld-Verified 榜上 Pointer、Intelligence-Indeed、OpenAPA、VLAA-GUI、HIPPO、Agent S3、OS-Symphony、UiPath、
GTA1、CoACT-1 的做法，全部放在**同一个 gui-gate MCP 服务**里（Rust，单个二进制），不加任何额外服务或客户端插件。
不设开关时行为与 0.9.6 完全相同。`GUI_GATE_097=1` 一次打开所有不需要模型的新功能，每项也可单开（`GUI_GATE_<名字>=1`）
或在总开关下单关（`=0`）：

| 开关 | 做什么（来源） |
|---|---|
| `INSPECT` | 新工具 `inspect`（唯一新增工具，只读）：kind=file 直接读文件（文本 / PDF / 办公文档存盘内容 / 图片原图）、env 可行性事实表、page 读 Chrome 标签页和网页文字（DevTools 端口）、region 截图局部放大、locate 按描述找元素、howto 怎么操作、memory 以往成功经验（Pointer、OpenAPA、VLAA、UiPath） |
| `JOBS` | `run_command` 加 `mode: run/start/poll/stop`：长任务后台跑、轮询、停止；start 照样过风险检查（Pointer） |
| `PHANTOM` | 模型编出来的工具名（triple_click、run_bash、type、key…）映射到真工具，否则给出最接近的工具名（Pointer） |
| `TWO_STRIKES` | 同一机制连续失败两次就要求换方法；同一界面状态反复出现 3 次要求换策略（Pointer、VLAA、OS-Symphony） |
| `FEASIBILITY_FIRST` | 任务简报 + 第一次 GUI 动作前必须先用 set_plan 给出能否完成的判断（只拦一次）（Pointer、II、HIPPO） |
| `STATE_PLAN` | set_plan 里程碑写「最终状态」并可带可检查条件（文件存在/改过/含某文本、窗口标题、单元格…），可中途修改；`facts` 工作记忆（Pointer、VLAA、HIPPO、UiPath） |
| `VERIFY_PLUS` | check_done 评估里程碑条件、读回已保存文件内容（VLAA、Pointer verifier） |
| `MICRO_VERIFY` | 保存后检查是否弹出格式/另存为对话框或标题仍有未保存标记；输入后检查文字是否出现（VLAA） |
| `SKILLS` | 内置分应用技能（原 bench 提示移入门内），随任务简报给出（Intelligence-Indeed） |
| `SNAP` | 像素点击落在所有元素之外但离某元素 10 px 内时，改点该元素（UiPath crop-and-refine 的无模型版） |
| `PICK` | `run_actions {pick_one:true}`：多个候选动作里按预测器挑最可能成功且安全的一个执行（GTA1、Agent S3 bBoN 的单步版） |
| `DIGEST` | 每 8 步附一份进度摘要（计划、事实、最近步骤）（Pointer 压缩、OpenAPA、HIPPO） |
| `GUI_GATE_MEMORY=文件` | 经验记忆：`gui-gate memory add --task-file T --log 门日志` 从成功运行生成；简报和 inspect memory 里给出相似任务的做法（HIPPO / muscle-mem）。评测时只能用不相交的任务集生成 |

**可选模型角色**（全部默认关闭，审核模型仍是风险步骤的唯一模型裁决者，干活的主模型从不批准自己的步骤）。v0.10.1 起 `planner`、`search`、`compact` 三个角色**即使配了接口也默认不启用**（它们以后可能移到客户端），要用须显式打开：`GUI_GATE_ROLE_<NAME>_ENABLE=1` 或 `GUI_GATE_ROLES_ENABLE=planner,search,compact`。角色列表：
`feasibility`（可行性判断，只读探测）、`planner`（最终状态里程碑）、`verifier`（终态核验，只读探测，每会话最多 2 次）、
`judge`（循环第 3 级 KEEP/SWITCH；pick_one 选择）、`search`（howto 教程）、`compact`（进度摘要）、`grounder`（视觉定位）。
配置同审核模型：`GUI_GATE_ROLE_<NAME>_URL / _MODEL / _KEY 或 _KEY_FILE / _PROTOCOL / _TIMEOUT / _MAX_TOKENS / _MAX_STEPS`，
或 `GUI_GATE_ROLE_<NAME>=reviewer` 直接复用审核模型的接口；也可写进 `reviewer.env`。探测型角色只能执行白名单里的只读命令
（ls、cat、find（不带 -delete/-exec）、dpkg -l、gsettings get、python3 -c 只读代码……，不能重定向、不能后台、不能命令替换），
还要再过门自己的风险检查。角色没配或调用失败时，功能退回无模型版本。

**工具清单**（core 工具集）：驱动工具 `list_apps list_windows get_window_state launch_app bring_to_front click double_click
right_click type_text press_key hotkey scroll set_value drag invoke_menu`；门工具 `request_human gate_task gate_report
run_actions run_command check_done set_plan`；0.9.7 开 `INSPECT` 时多一个 `inspect`。关掉的功能不出现在 tools/list 里；
`tests/test_v097.py` 固定了默认和全开两种情况下的工具列表。

基准：`PI_TOOLS_097=1 PI_APP_HINTS=0 bash bench/run_bench.sh ...`（门内技能取代 bench 提示）。

## v0.4 新增（借鉴 Claude Code auto mode / Codex auto-review，见 docs/）

- **看用户原话授权**：只认启动者给的 `--task` / `--task-file` / 环境变量 `GUI_GATE_TASK`。原话明确说了「覆盖 / 删除 / 移动……」
  并且点名了这个效果作用的每一个文件（写了路径就必须是这个路径），风险步骤直接放行；覆盖当前正在编辑的文档本身也放行；
  没有可匹配对象的效果（发送按钮、删除控件……）交给审核模型或人。Agent 调用的 `gate_task` 只作上下文，**从不授权**。
  安全模型和强制位置见 [docs/security-model.md](docs/security-model.md)。
- **拦了接着干**：被拒时返回理由 + 安全做法（换名保存、跳过冲突文件、先列出再执行……），
  同一后果换条路做也会被拒；连续 3 次 / 累计 8 次被拒才让 Agent 停下汇报。
- **独立审核模型**（门里唯一的大模型）：中风险步骤和拿不准的目标都由它判，只看「用户原话 + 具体动作 +
  界面树/对话框」，看不到对话和主模型的理由。审核只用你自己配置的独立模型，通过 API 调用，且**必须配置**：
  - 配置（环境变量或 `~/.config/gui-gate/reviewer.env`，另一个文件用 `GUI_GATE_REVIEWER_ENV` 指定）：
    必填 `GUI_GATE_REVIEWER_URL`、`_MODEL`、`_KEY`（或 `_KEY_FILE`）；可选 `_USER_AGENT`、`_MAX_TOKENS`、`_TIMEOUT`、
    `_PROTOCOL`。URL 可以填基础地址（`.../v1`）或完整接口地址。支持的协议（`_PROTOCOL`，默认 `auto` 按地址和模型名猜）：

    | 协议 | 用于 | 请求 |
    |---|---|---|
    | `systemone` | 决策模型：TypeSafe Jev、千问/百炼 `decision-model-preview`、Kev、Ollama、Upstage Solar Decide 等 | `POST …/systemone`，发 state + 是非题，返回概率；每题 P ≥ `GUI_GATE_REVIEWER_ALLOW_P`（默认 0.5）才放行 |
    | `chat` | OpenAI Chat Completions 及兼容接口（DeepSeek、千问、各类中转） | `POST …/chat/completions` |
    | `responses` | OpenAI Responses | `POST …/responses` |
    | `anthropic` | Anthropic Messages（Claude 及兼容接口） | `POST …/v1/messages` |
    | `gemini` | Google Gemini | `POST …/models/<model>:generateContent` |

    配好后用 `gui-gate check-reviewer [--cases FILE]` 自检：会显示用的是哪个模型、哪种协议，并跑一条该拒、一条该放的样例。
  - 没配审核模型（缺 URL / 模型 / 密钥，密钥文件读不到或为空，或设了 `GUI_GATE_REVIEWER=off` 之类）：
    门拒绝启动并说明怎么配；`gui-gate check-reviewer` 报告「not configured」。
    运行中审核 API 出错或超时（fail closed）：有人可找就问人，无人值守就拒绝并给出安全做法。
  干活的主模型从不审核自己的动作，门也不会借用 Agent 自己的模型。
  独立审核模型是**必要的防线之一，单靠它不够**（one necessary line of defence, not sufficient on its own）：
  独立调用不等于错误独立，界面里的提示注入可能同时误导主模型和审核模型；所以授权只来自启动者原话和人，
  拒绝记录、一次性许可和最终派发检查不依赖审核模型的判断（见 docs/security-model.md）。
- **省观察**：截图只在刚用过坐标点击时附带；动作后紧接着重读同一窗口直接用缓存（`fresh:true` 强制重读）；
  表格里「粘贴覆盖单元格」不再当成覆盖文件。
- **服务端批量执行（任何 Agent 可用）**：`run_actions` 一次传一段小程序：动作步骤 + find（找元素）/
  for_each（逐个处理）/ if（有没有某元素）/ read（读出值）。每个 GUI 动作单独过门；被拒 / 报错 /
  意外弹窗 / 期望不符就停，整批结束只观察一次；坐标点击只允许第一步；后出现的菜单项、按钮按名字找；
  最多 20 个动作、嵌套 3 层，不能执行任意代码。

## 安装

**只下载一样东西**：每个平台一个 gui-gate 发布包，里面是 gui-gate 本体 + 它测试过的 Cua Driver 0.38.0
（Cua 官方原版构建，MIT；发版时已核对 Cua 的 SHA256SUMS 和 Sigstore 签名，见 NOTICE / `scripts/cua-driver.lock`）。
安装器校验 SHA256 → 放好两者 → 加 PATH → 自检（`gui-gate doctor`）。**不再运行 Cua 的安装器**（因此没有 Windows
开机计划任务、没有 junction、没有 Cua 的 PATH 行和安装遥测）。不需要 Python、Rust 或管理员权限。

```bash
curl -fsSL https://github.com/cszf1/gui-gate/releases/latest/download/install.sh | bash          # macOS / Linux
irm https://github.com/cszf1/gui-gate/releases/latest/download/install.ps1 | iex                  # Windows PowerShell
```

非交互（例如脚本里）直接给审核模型：
`curl -fsSL .../install.sh | GUI_GATE_REVIEWER_KEY=sk-... bash -s -- --reviewer-url https://api.deepseek.com/v1 --reviewer-model deepseek-chat`
（Windows：先设 `$env:GUI_GATE_REVIEWER_URL/_MODEL/_KEY` 再 `irm ... | iex`）。

| 选项（sh 参数 / 环境变量） | 作用 |
|---|---|
| `--client claude` / `GUI_GATE_CLIENT=claude,codex` | 顺便写入 Agent 配置 |
| `--version 0.10.1` / `GUI_GATE_VERSION` | 装指定版本 |
| `--bin-dir` / `GUI_GATE_INSTALL_DIR` | gui-gate 目录（默认 `~/.local/bin`，Windows `%LOCALAPPDATA%\Programs\gui-gate\bin`） |
| `--lib-dir` / `GUI_GATE_LIB_DIR` | 内置驱动目录（默认 `<bin>/../lib/gui-gate`；Windows 固定 `<bin>\..\cua-driver`） |
| `--skip-driver` / `GUI_GATE_SKIP_DRIVER=1` | 不装内置驱动，改用你另外装的 Cua Driver |
| `--no-modify-path` | 不改 shell 配置文件 |
| `GUI_GATE_BASE_URL` | 从镜像下载（GitHub 慢时） |

运行时：`GUI_GATE_DRIVER=/path/to/cua-driver` 指定驱动，`GUI_GATE_DRIVER=external` 跳过内置驱动、用 Cua 官方
安装器装的那份。内置驱动启动时 gui-gate 关掉它的遥测和自更新检查（`GUI_GATE_DRIVER_TELEMETRY=1` 恢复 Cua 默认）。

平台：macOS（universal）、Linux x86_64 / aarch64（gate 静态链接；驱动为 Cua 官方 glibc 构建）、Windows x64。
macOS 上驱动是 Cua 签名的 `CuaDriver.app`，原样放到 `/Applications`（不可写时 `~/Applications`），所以辅助功能 /
屏幕录制权限和用 Cua 安装器装的一样；如果你已经有 `/Applications/CuaDriver.app`（不是 gui-gate 装的），保持不动、直接用它。

**更新**：重新跑安装命令，或 `gui-gate update --self`。内置驱动只随 gui-gate 发版一起变；`gui-gate update` 只报告
有没有新的 gui-gate 版本，不会单独去拉新驱动，后台也不联网查驱动版本（只有用外部驱动时才保留旧的每日检查，
`GUI_GATE_NO_UPDATE_CHECK=1` 关闭）。每次安装/更新后自动运行 `gui-gate cleanup`，清理旧版本残留（旧副本、
0.2–0.5 的 Python 包、重复/失效的 PATH 行或 Windows 用户 Path 项、指向已删副本的 MCP 配置和已删参数、gui-gate
自己的配置备份、旧缓存、旧日志、死掉的 gate 留下的临时文件）；`gui-gate cleanup --dry-run` 先看会动什么。

### gui-gate 在你电脑上放了什么（完整清单见 docs/footprint-0.9.7.md）

| 东西 | 位置 |
|---|---|
| 本体 | `~/.local/bin/gui-gate`；Windows `%LOCALAPPDATA%\Programs\gui-gate\bin\gui-gate.exe` |
| 内置 Cua Driver + 其 LICENSE、安装清单 `install-manifest` | Linux/macOS `~/.local/lib/gui-gate/`；Windows `%LOCALAPPDATA%\Programs\gui-gate\` |
| macOS 驱动 App（仅当原来没有时） | `/Applications/CuaDriver.app` |
| PATH | 只改一个 shell 配置文件里的 `# gui-gate` 两行（仅当目录不在 PATH 时；zsh `~/.zshrc`，Linux bash `~/.bashrc`，macOS bash 用已存在的 `~/.bash_profile`/`~/.bash_login`/`~/.profile` 中第一个；文件原本不存在时新建并记入安装清单，卸载时若只剩我们的行就删掉）；Windows 用户 Path 一项 |
| 审核模型设置与密钥（600 权限）、角色密钥、经验记忆 | `~/.config/gui-gate/` |
| 会话日志（含任务原文和命令；启动时清理 30 天前 / 超过 200 MB 的） | `~/.gui-gate/logs/`（`--log none` 不写） |
| 外部驱动时的版本检查缓存 | `~/.cache/gui-gate/` |
| MCP 配置项（只在你 `setup --client` 时） | `~/.claude.json`、`~/.pi/agent/mcp.json`、`~/.cursor/mcp.json`、`~/.codex/config.toml`，不留 .bak |
| 临时文件 | `$TMPDIR/gui-gate-*`（后台任务日志在 Unix 上一启动就删名；gate 退出时清掉自己的） |
| 进程 | 由 MCP 客户端启动的 `gui-gate` 和它的 `cua-driver mcp`（macOS 上还有 CuaDriver.app 的守护进程）；客户端关掉 stdin 或发 SIGTERM/SIGINT/SIGHUP 时，gate 结束所有后台任务和仍在跑的命令（整组进程）再退出；`gui-gate uninstall` 删 CuaDriver.app 前先停掉并确认它的所有进程都已退出 |

没有服务、计划任务、LaunchAgent、systemd 单元，也不改浏览器配置（`inspect kind=page` 只在 Chrome 调试端口
本来就开着时只读连接本机）。

### 卸载

```bash
gui-gate uninstall                 # 删除上表全部：本体、内置驱动（macOS 只删 gui-gate 自己装的 CuaDriver.app 及其权限记录）、
                                   # PATH 行 / 用户 Path 项、MCP 配置项、~/.config/gui-gate、~/.gui-gate、~/.cache/gui-gate、
                                   # 临时文件、安装器新建且已空的目录
gui-gate uninstall --dry-run       # 先看会删什么
gui-gate uninstall --keep-config   # 保留审核模型设置和密钥；--keep-logs 保留日志
gui-gate uninstall --with-driver   # 另外用 Cua 自己的卸载器（--purge）删掉你用 Cua 官方安装器装的驱动，并删它留下的 PATH 行
```

Windows 同样是 `gui-gate uninstall`（exe 会先改名，几秒后由一个隐藏的后台命令删掉并移除空目录）。
用 Cua 官方安装器装的驱动默认不动；手动删：macOS/Linux `bash -c "$(curl -fsSL https://cua.ai/driver/uninstall.sh)" -- --purge`，
Windows `$env:CUA_DRIVER_RS_UNINSTALL_PURGE='1'; irm https://cua.ai/driver/uninstall.ps1 | iex`。

Linux 桌面需要开启无障碍（AT-SPI，多数发行版默认有）；macOS 首次使用要给辅助功能 / 屏幕录制权限。

常用参数：

```bash
gui-gate --policy gate            # 默认：价值门
gui-gate --policy direct          # 基线：从不干预（只记录）
gui-gate --policy light|full      # 基线：每步前必须读树 / 必须截图
gui-gate --lambda 2               # 更省（少看）；0.5 更稳
gui-gate --collect                # 采数据：每个动作做影子标注
gui-gate --weights weights.json   # 用训练好的预测器
```

运行中模型也可以调用 `gate_report`（本次统计）和 `gate_config`（改策略/λ）。

## 结果怎么读

v0.9.9 的 OSWorld-Verified 运行（`results/osworld-verified-v099-ds41-2026-10-23/`）是**基于 OSWorld 任务与评估器的自定义环境**
（custom environment based on OSWorld tasks and evaluators, not comparable to the leaderboard：原生 Debian/XFCE、无 VM、应用版本和预算不同）
上的部分结果，is not an official OSWorld-Verified score。按提交的数据：有效运行 185/332 = 55.7 %（Wilson 95 % 区间 50.3 %–61.0 %，
未计任务相关性与有效样本选择）；按计划的 361 题算 185/361 = 51.2 %（Wilson 46.1 %–56.4 %，基础设施失败 / 缺失记 0）。
不要和榜单数字直接比较。评测协议见 docs/eval-protocol.md 和 bench/README.md。

## 采数据 → 训练 → 对比

```bash
gui-gate --policy direct --collect --log runs/s1.jsonl     # 不干预地采集，标签无偏
gui-gate train runs/ -o weights.json --split run+family --report   # 按运行和任务族同时留出；Brier/NLL/等质量分箱 ECE + 自助法区间 + 可靠性表
gui-gate train --model treatment runs/ -o value.json --report       # 处理交互价值模型（日志里实际采取的选项）
gui-gate costs runs/ -o costs.json                                  # 各选项实测代价（GUI_GATE_COSTS）
gui-gate compare runs/                                     # 各策略：执行数、失败率、观察/截图数、门开销
gui-gate report runs/                                      # 问人负担：每任务询问数、重复比例、等待/主动秒数、接管步骤、无回答率
```

日志默认写到 `~/.gui-gate/logs/`，每行一个事件（JSONL）。

## 试验台

`scripts/fixture_app.py`：GTK3 测试程序，带改名、加载、弹窗三种真实干扰，真值单独记录用于评分。
`scripts/sim_agent.py`：**脚本化**的 Agent（不是大模型），用来在真实桌面上把整条链路跑通。
`scripts/run_matrix.sh`：同一批种子在各策略下跑，加上采数据和训练。

`weights/osworld-2026-10-21.json`：用 OSWorld/OSGuard 实测日志（1948 个直接执行的动作，118 个会话）训练的预测器，
按会话留出 35 个会话测试：手写先验 AUC 0.50（等于瞎猜）、ECE 0.096；训练后 AUC 0.66、ECE 0.037。
默认仍用先验，需要时 `--weights weights/osworld-2026-10-21.json`（还没在基准上验证过）。

## 开发

```bash
cargo build --release                       # 源码在 src/（Rust）
cargo test                                  # 模块级单元测试
GUI_GATE_BIN=target/release/gui-gate python3 -m unittest discover -s tests -v   # 用假 driver 走真实 MCP stdio
maturin build --release                     # 打包成 Python wheel（发布由 .github/workflows/release.yml 按 tag 自动做）
```

源码：`server.rs` 门控主流程 · `decide.rs` 规则与预测器 · `features.rs` 特征与风险 · `sidefx.rs` 文件系统副作用检查 ·
`authz.rs` 用户原话授权 · `batch.rs` 批量执行 · `human.rs` 找人确认 · `reviewer.rs` 独立审核员 ·
`install.rs` / `updates.rs` 安装、自检、驱动查找与更新 · `cleanup.rs` 清理与卸载 · `proc.rs` 子进程与退出清理 · `tools.rs` 训练与对比 · `value.rs` 价值模型 / 实测代价 / 信息价值 / 固定策略 · `valuetrain.rs` 价值模型训练与评估 · `shot.rs` 截图转 JPEG。

## 许可

MIT。Cua Driver（Copyright (c) 2025 Cua AI, Inc.）为 trycua/cua 的 MIT 项目；发布包里附带其官方原版构建，许可证随附在
`cua-driver/LICENSE` 和 `third_party/cua-driver/`，见 NOTICE。gui-gate 只通过其 MCP 接口以子进程方式调用它。
