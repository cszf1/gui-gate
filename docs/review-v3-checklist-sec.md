# v0.10.3 安全 / 问人 / 角色清单（审核 2026-10-26 v2，第 1 节末状态机建议 + 第 3、4、7 节）

分支 v3-sec（基于 main acaba69）。研究 / 预测器部分（第 2、5、6 节，x_pre/x_post、价值选择、online_plan）在 v3-res，本清单不含。
每条：审核原话要点 → 门里怎么改 / 为什么不能在门里改 → 回归测试（真门二进制 + tests/fake_driver.py，断言驱动实际收到的动作）。
状态：✅ 已改并有测试；📝 只能文档说明（写明原因）；↪ 属于客户端/其他分支。

## 第 1 节末：连续失败—换策略—求助—停止 状态机

| # | 要点 | 处理 | 状态 |
|---|---|---|---|
| 1.1 | 局部模型之外加明确的"连续失败→换策略→求助→停止"状态机，暂不上完整 RL | 门里已有 loopbreak（级别 0–3，纯规则）。本轮把它命名为显式状态并放进结构化结果：`loop_state` = `normal`→`switch_strategy`（级别 1：换操作方式）→`blocked_repeat`（级别 2：拒绝重发无进展的同一步）→`help_or_stop`（级别 3：有人→ask_help，无人→停止并报告）。状态迁移和阈值不变（不在本分支调参，避免拟合 361 题）。文档 gate-design-v2 §状态机。 | ✅ |
| 1.2 | 安全必需的检查不参与期望收益竞争 | 已有 safety_first + safety_decision_ok 断言；本轮第 5 条把"价值模型选 review 但审核失败"也改成规则路径（有人问人/无人拒绝），不再回退到分数最低的 direct。 | ✅（见 3.4） |

## 第 3 节：安全边界

| # | 要点 | 处理 | 状态 |
|---|---|---|---|
| 3.1 | 后台输入回退第二次直接调 tool_raw，绕过最终检查 | `Gate::tool` 改成单一发送循环：每一次真正发给驱动的动作（首次、后台→前台回退、焦点修复、遮挡修复、批量内部步骤）都先过同一个 `send_checks`（冻结检查、自身对话框检查、最终派发检查 + 审批前提重读）。只有驱动明确说"未送达"（`not_delivered()`：后台不可用 / 前台不可用 no input was sent / 指针点被别的控件占有 / 无可用无焦点动作）才允许自动重试；重试前把本次刚消耗的一次性许可"退回"，再完整走一遍检查（目标、效果键、任务版本、审批前提都重新算），变了就拒绝。gated() 里的遮挡 / 焦点修复重试同样先判 `not_delivered`。 | ✅ |
| 3.2 | unknown 只是暂时挡住：重新观察前就移除；读窗口失败也清掉 | unknown 改成账本 `UnknownEntry{why, at, pid, window, non_idempotent}`：只有在 `at` 之后对同一窗口有一次**成功**读取才算"重新观察过"；读失败不清记录、继续拒绝。可幂等效果读过后才解锁；非幂等效果（有风险标签 / 交接目标：发送、删除、支付、提交…）读过也不自动解锁，需要真人核实（有人→问人"它是否已经发生"，批准才发送；无人→拒绝），记录跨 gate_reset 保留。驱动没有幂等键、门也拿不到"确定未执行"的证据，所以门里唯一可用的解锁证据是真人核实——写进 security-model。pass-through 动作同一套。 | ✅ |
| 3.3 | 审批只绑定控件/窗口/点击参数，不绑定收件人、正文等业务前提 | 批准时记录 `Premise`：目标窗口最近一次树读取里所有带值/文本的行（收件人、金额、正文、表单字段、标签）的摘要 + 快照 ID（对象版本）+ 效果涉及文件的 (mtime,size)；问人表单 `_meta.premise` 给出摘要，答案绑定里含前提摘要。派发前（最终检查里）重读目标窗口（树读取，令牌按 role+label 重新绑定）和文件状态，摘要变了 → 许可作废、清审批，返回 `premise_changed`，重发时重新问人/审核。读不到关键状态（无树、无文件）时如实标 `premise: unverifiable`：表单里写明"不可验证"，许可有效期缩到 10 s，且有人在场时审核模型的批准不够、改问人。拒绝键与批准键继续分开：批准严格一次性、30 s（不可验证 10 s）；拒绝记录按资源范围覆盖。basename 回退（对话框只给文件名时 `name:` 与路径同名匹配）和"同应用同标签"是**保守近似**：只用于拒绝覆盖（多拒不多放），批准从不用它们——写进 effect.rs 模块注释和 security-model。 | ✅ |
| 3.4 | 价值模型选 review、审核无响应时会选其他分数最好的选项（可能 direct） | review_step 审核失败时：有真人通道→ask_human（真人批准才执行），无人（--human deny）→拒绝（不执行，返回 tier review_unavailable）。规则路径原样（有人 ask_help / 无人 stop_loop）。风险路径本来就 fail closed。 | ✅ |
| 3.5 | 效果 payload 摘要用 FNV-1a 64 | 改为 SHA-256（门里已有经 NIST 向量测试的纯 Rust 实现 util::sha256，HMAC 也用它；没有再引第三方 crate，原因：零新依赖、aarch64 + zig cc 不涉及 C/汇编，与 sha2 crate 输出一致，测试向量对拍）。`EffectKey::from_json` 读旧日志：16 位十六进制标为 `fnv1a64` 旧摘要，只用于显示/统计，不参与新的等价判断。 | ✅ |
| 3.6 | requestState HMAC 的文档概念错误 | security-model / gate-design-v2 改写：HMAC 只证明状态由本门进程签发且未被改，不证明 allow 来自真人；经手状态和答案的客户端扩展属于可信计算基。 | ✅ 📝 |
| 3.7 | 威胁模型 | 写明"合作但会犯错的 agent"；关键词规则和独立审核不是恶意代码沙箱（同用户运行，换解释器/分步构造/GUI 自动化都能绕过），真正隔离要靠 OS 级沙箱（未来工作）。攻击集全过 ≠ 无绕过。安全测试与成功率分开报告。 | 📝 |
| 3.8 | 攻击测试断言驱动实际收到什么 | fake_driver 新增 `FAKE_ACTION_LOG`（驱动收到的每个动作写一行，崩溃前也写，重启后不丢）和 `FAKE_CTL`（模拟用户在门外改界面/焦点）；本轮所有新测试都按这份记录断言次数和内容。 | ✅ |

## 第 4 节：真人交接

| # | 要点 | 处理 | 状态 |
|---|---|---|---|
| 4.1 | note_burden 把等待时间记成 active_s | 分开：`wait_s` = 弹窗到回答；`active_s` 只有客户端给出交互证据（回答 `_meta["io.github.cszf1/gui-gate"].active_ms`）才记；没有就记缺失（`active_s: null`），另给上界 `active_upper_s = wait_s`。汇总里 active_measured / active_missing 分开计数。 | ✅ |
| 4.2 | MRTR 无重试过期的请求没进 unanswered，同一效果会再弹 | 过期清扫时按请求记录里的 ask_key + 任务版本写入 unanswered，同一效果在同一任务版本内不再弹窗（返回 not_asked_again）。 | ✅ |
| 4.3 | 每个请求统一记录、唯一终态 | 新增 `person_request` 记录（req、kind、task_version、effect、deadline）和唯一收尾函数 `req_end`（`person_request_end`，终态之一：answered / no_answer / expired_no_retry / late / void / disconnected / unused）。过期、断连（gate 退出）、迟到、reset 作废、答案未被使用都走它；重复收尾被忽略并计数。负担统计只在收尾时记一次。 | ✅ |
| 4.4 | 接管完成后重读受影响状态，禁止自动重放 | 接管 / 求助"我已做"之后：重读目标窗口树、窗口列表（谁在前台）、效果涉及的文件状态，结果放 `structuredContent.reread`；清掉所有许可和审批；记录 `takeover_at`，之后任何基于接管前读取的令牌/索引/坐标都按"状态已变"处理（先读再发，不自动重绑）；同一动作重发进 unknown 账本（可幂等：先读一次；非幂等：真人核实）。门从不重放原动作。测试含用户切窗口、改文件、改焦点。 | ✅ |
| 4.5 | 安全必需批准不因问人收益低跳过；可选澄清/求助才做价值判断 | 已是这样（safety_first 不走价值模型）；文档写明。 | 📝 |
| 4.6 | 用户主观打扰程度、接管完成率等指标 | 门只能记录客观事件（请求、终态、等待/主动时间、交还后重复动作）；主观打扰需要客户端问卷——属于桌面客户端。 | ↪ 客户端 |

## 第 7 节：角色

| # | 要点 | 处理 | 状态 |
|---|---|---|---|
| 7.1 | role_enabled 对 feasibility/verifier/judge/grounder 直接返回 true，`_ENABLE=0` 关不掉 | 优先级统一：显式 `GUI_GATE_ROLE_<NAME>_ENABLE=0` > 组开关（`GUI_GATE_ROLES_ENABLE`）> 端点存在。七个角色同一函数。 | ✅ |
| 7.2 | compact 门内默认关；planner/search 默认关（移到客户端） | 三者默认关（显式 `_ENABLE=1` 才开）；文档说明它们应在客户端实现。 | ✅ 📝 |
| 7.3 | verifier 只验证明确后置条件 | 只有存在明确后置条件（计划里程碑或带机器检查的谓词）时才调用 verifier，提示词只给这些后置条件，要求逐条给证据；没有就不调用，check_done 只给门自己的证据。 | ✅ |
| 7.4 | check_done/verifier 1938 s 无返回 | 角色调用总截止时间 `GUI_GATE_ROLE_DEADLINE_S`（默认 120 s，单角色 `GUI_GATE_ROLE_<NAME>_DEADLINE_S`）：跨所有子调用和只读探测；每次 HTTP 的超时取 min(单次超时, 剩余时间)；到点返回结构化 `{"status":"timeout"}`，不挂起。 | ✅ |
| 7.5 | 主模型 / 审核模型单次 HTTP 超时 | 审核模型：ureq 整体超时（默认 15 s，`GUI_GATE_REVIEWER_TIMEOUT`），本轮补测试。主模型不经过门（由 Pi/桌面客户端调用），单次请求超时只能在客户端设——写进文档。 | ✅ / ↪ 客户端 |
| 7.6 | 结构化执行状态 | 所有工具结果 `structuredContent.status` ∈ done / partial / unknown / input_required / done_by_person / blocked（原 executed 改名 done，并保留 `executed` 布尔/"unknown" 字段兼容旧客户端）；附 `continuation_id`、批量的 `completed_steps` / `next_step`、`postcondition`（驱动给的效果证据：verified / unverifiable / none）。 | ✅ |
| 7.7 | 打包、Windows/macOS 真机、卸载边界 | 属于桌面客户端发布流程，不在门里。 | ↪ 客户端 |

## 回归测试对照（tests/test_v3_sec.py，20 个；在 v0.10.2 = acaba69 的二进制上 19 个失败）

| 条目 | 测试 | 在 v0.10.2 上的失败（修前证据） |
|---|---|---|
| 3.1 | `EverySendIsChecked.test_task_change_between_background_failure_and_foreground_retry` | 驱动收到 2 次 hotkey（第二次 `delivery_mode: foreground`，任务已换版本仍送达） |
| 3.1 | `EverySendIsChecked.test_unchanged_foreground_retry_runs_and_is_checked` | 两次发送只有 0 次 `send_check` |
| 3.2 | `UnknownLedger.test_unmodelled_action_failed_window_list_does_not_clear` | 无模型目标路径（set_clipboard）：记录在读窗口列表之前就删了，列表读失败后第二次重发送达驱动 |
| 3.2 | `UnknownLedger.test_failed_read_does_not_clear_the_record` | （v0.10.2 上也通过：GUI 路径另有“同一动作刚失败 → look_full”规则，读失败时一直挡着；保留为不回退检查） |
| 3.2 | `UnknownLedger.test_non_idempotent_needs_a_person_even_after_a_read` | 读过窗口后无人核实就重发 |
| 3.2 | `UnknownLedger.test_person_confirms_then_it_is_sent` | 没有问人（p.asked 为空） |
| 3.2 | `UnknownLedger.test_idempotent_step_unlocks_after_a_successful_read` | （可幂等步骤读过后照常执行：防止过度拦截） |
| 3.3 | `ApprovalPremise.test_recipient_changed_after_approval_voids_it` | 批准给 bob@ 后收件人变 eve@，驱动仍收到 1 次点击 Send |
| 3.3 | `ApprovalPremise.test_unreadable_premise_is_said_so` | 表单里没有 premise |
| 3.4 | `ReviewFailureNeverDirect.test_unattended_is_refused` / `test_attended_asks_the_person` | 审核不答时直接执行（驱动收到第二次点击）；有人时也没问人 |
| 4.1 | `PersonRecords.test_wait_is_not_active_time` / `test_client_measured_active_time` | active_s = 等待时间 1.2 s；客户端测得的 0.4 s 被忽略 |
| 4.2/4.3 | `PersonRecords.test_expired_without_retry_is_not_asked_again_and_ends_once` | 同一效果再次 input_required；没有 person_request 记录 |
| 4.3 | `PersonRecords.test_reset_void_then_answer_ends_once` | 没有唯一收尾记录 |
| 4.4 | `PersonRecords.test_takeover_rereads_and_never_replays` | 没有 reread；接管前的令牌在人改过界面后仍被重绑执行 |
| 7.1 | `Roles.test_explicit_disable_wins_over_endpoint` | `_ENABLE=0` 时 verifier 仍收到请求 |
| 7.4 | `Roles.test_role_calls_have_a_total_deadline` | check_done 27 s（截止 4 s） |
| 7.3 | `Roles.test_verifier_only_with_explicit_postconditions` | 无后置条件也调用 verifier |
| 7.6 | `StructuredStatus.test_status_values_and_fields` | status 仍是 executed，没有 executed/postcondition/continuation_id/loop_state |

Rust 单元测试：`effect::tests::digests_differ` / `old_logs_still_read`（3.5），`roles::tests::explicit_disable_wins_for_every_role`（7.1），
`reviewer::deadline_tests::request_timeout_is_capped_by_the_deadline`（7.4/7.5），`ledger::tests::*`（3.3 前提摘要、文件版本、不可验证）。
