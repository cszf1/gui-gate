# 审核清单：研究 / 预测器 / 评测部分（v0.10.3，分支 v3-res）

来源：外部审核 2026-10-26 第二版（复核 gui-gate v0.10.2 acaba69 / gui-pi-desktop v0.3.1 8d3eb86）第 1、2、6 节和第 8 节第 2、3 项。
安全部分（第 3、4 节，第 8 节第 1 项：派发、unknown、审批前提、human.rs、roles.rs、effect.rs）由并行分支 v3-sec 处理，不在本清单。

约束：默认决策与 acaba69 完全一致——`value::tests::default_path_output_is_unchanged`（golden 文件
`src/testdata/value_default_golden.json`，含决策和 value 日志）未改动且通过。本分支没有做任何真实模型 / 在线测试，
没有修改任何已有 results 目录（只新增 `results/offline-v3-2026-10-26/`、`results/online-plan-v3-2026-10-26/`）。

状态：**已改** = 代码 + 测试 + 文档；**已有** = acaba69 已满足，只补文档；**待在线验证** = 代码/计划已就绪，结论需要配对分支或在线数据；
**待用户** = 需要人工判断；**未做** = 说明原因。

## 第 1 节 研究与工程：优化目标和研究对象

| # | 审核意见 | 状态 | 做了什么 / 为什么只能待验证 |
|---|---|---|---|
| 1.1 | 公式 E[cost]=λc+(1−P)C_fail 隐含三假设（P_o 同一件事、失败压成一个数、介入后难度不变） | 已改（文档） | docs/value-model-v2.md §9：产品目标 Q^π(h,b,o) 与局部净收益 G_o 两层分开写，说明当前模型只估 G_o 的执行层 Δp |
| 1.2 | 两层目标：Q^π = E[L(1−Y)+λT+γH+ηN \| do(o)]，只在 A_safe 内选 | 已改（文档） | §9 表格写明每个量的来源；L、λ、γ 由产品/预注册给定，日志学不出来 |
| 1.3 | G_o = C_recover·Δp − λΔt − γΔH − η·1[o≠direct]，Δt 只算额外前置耗时 | 已改 | `research::local_gain` + 测试 `local_gain_does_not_count_recovery_time_twice`；treatment 路径改用 G_o（`value::treatment_choose`），Δt 来自 `value::cost_split`（额外前置秒，不含失败恢复） |
| 1.4 | 加"连续失败→换策略→求助→停止"状态机 | 已改（只产生建议） | `research::escalation`：≥2 换策略，≥4 或回路级别≥2 求助（有人可达时），≥6 停止；写进每个 action 事件 `escalation` 和 gate_next_step 返回；**不改变任何决策**，安全必需的批准不经过它（与 v3-sec 的问人逻辑无交叉） |
| 1.5 | deny / 授权 / 密码接管 / 安全检查不和可选观察竞争期望收益 | 已有 | server.rs `safety_decision_ok` 断言保留；文档 §9 写明 |
| 1.6 | 标签三层：执行 / 局部进展 / 任务，不互相冒充 | 已改 | 日志 `labels.exec/local/task`（`research::exec_labels`，task 在门里恒为 null）；训练行 `TRow.y`（执行层）/`y_local`/`y_task` 分字段 |
| 1.7 | 不把任务成败复制给所有动作当"动作价值标签" | 已改 | `gui-gate train --model treatment --label task` 拒绝（退出码 2，说明原因）；`load_rows` 只把任务结果放 `y_task`；测试 `task_label_is_refused_and_dr_recovers_a_known_effect` |
| 1.8 | 暂不上完整 RL | 已有 | 文档 §9 写明 |
| 1.9 | 核心检验：同一可恢复状态分支各选项，检验 τ̂_o；画增量校准图（允许负增益） | 代码已改，**待在线验证** | `bench/offline_v2.py incremental_calibration`（按预测 τ̂ 分箱 vs 配对分支实际差，检查点内先平均，允许负值，检查点自助法区间）；现在没有配对分支数据，报告输出空并标"no paired branch data"；数据由增量试点（216 分支）产生 |
| 1.10 | 最后用完整在线任务验证策略收益，风险 AUC 不能替代 | **待在线验证** | 策略试点计划（108 次运行）已生成，未运行 |

## 第 2 节 预测器：数据定义

| # | 审核意见 | 状态 | 做了什么 / 原因 |
|---|---|---|---|
| 2.1 | 日志混入处理后特征（check/wait/heal 后重算 f，事件记录的是重算后的 f） | 已改 | 每个 action 事件新增不可变 `x_pre`（第一次决策时的特征，之后从不修改）、`x_post`（=旧 `features`）、`decision_id`、`contract: decision_v1`；测试 `test_heal_keeps_the_pre_decision_state`（x_pre.token_stale=1，x_post=0） |
| 2.2 | wait 后把 loading_cue、last_popup 直接清零；waited=1 与 loading_cue=0 分开 | 已改（日志/训练）；默认决策保持 | 默认重新决策仍清零（改了就会改变默认决策，违反"与 acaba69 完全一致"）；但 `x_pre` 保留原值，`x_post_observed` 记录 wait 后实际观察到的值，`assumed_resets` 标出按假设清零的键，`waited` 单独一个字段；训练只读 x_pre |
| 2.3 | 训练端只用 x_pre；无法恢复处理前状态的历史样本排除出因果估计，仅作描述统计，并在报告列数量 | 已改 | `valuetrain::feature_key/pre_state_recoverable`：有 x_pre 用 x_pre；旧日志只有在 trail 里没有 wait/check/heal 时 `features` 才等于处理前状态，否则排除，计入 `pre_decision_state_unrecoverable`（`--include-post` 只用于描述）。v0.9.9 运行的数量见下方"数字" |
| 2.4 | hand-back 标签归属过宽（180 s 内多个介入都用下一次执行结果） | 已改 | 新日志：`continues`（续接 decision_id、same_candidate、same_effect、model_switched、attributable），只有 attributable 才给标签；旧日志：同一签名才归属；其余记"无法归属"（`handback_unattributable`），不给标签。测试 `x_pre_continuations_and_component_costs_of_the_v0103_contract`、`test_handback_is_linked_to_its_continuation`、`test_a_switched_action_is_linked_but_not_attributable` |
| 2.5 | 记录原候选动作、效果标识、完整处理链、续接 ID、处理起止时间；区分提议/实际/模型换成了什么 | 已改 | `candidate{tool,sig,effect_id}`、`chain[{option,t0_ms,t1_ms,result,rebound,includes_repair}]`、`exec_t0_ms`、`proposed` vs `decision`、`continues.model_switched` |
| 2.6 | 运行时 covered(o) 是选项级全局门槛，不检查当前状态 | 已改 | 训练器总是拟合并保存记录策略倾向分（`propensity`，`--support-clip` 默认 0.05）；`TreatmentModel::covered_at(f,o)` 要求 e_o(x)≥clip 且 e_direct(x)≥clip 且有效行数≥min_rows；否则日志 `no_estimate_basis` 写原因；没有保存倾向分的模型除 direct 外一律无依据。测试 `coverage_is_checked_per_state_and_no_basis_falls_back_to_the_legacy_heuristic` |
| 2.7 | 未覆盖时明确"没有估计依据"，不要悄悄退回手工增益还叫 learned | 已改 | 全部无依据 → 旧先验决定，日志 `fallback: "legacy_heuristic"`、`learned: false`、`model: "legacy_prior"`；bootstrap 不足的旧回退也改成同样字段 |
| 2.8 | softmax 给非零概率不等于原确定性策略真会选另一条路 | 部分（文档） | 状态级双边 clip 是能做的最低要求；文档 §10.3/§10.5 写明它仍是观察性估计，**待在线验证**（配对分支） |
| 2.9 | 选项比较用净收益保守下界，不是先选点估计最大再检查 | 已改 | `treatment_choose`：每个有依据的选项算 G_o 的 bootstrap 百分位区间，取下界最大者且下界>0，否则 direct（`tier: model_lb`，日志 `point_best` 记点估计会选什么）；对所有选项（含 check、风险步骤）一致。测试 `options_are_compared_on_the_lower_bound_not_the_point_estimate` |
| 2.10 | bootstrap 复用原 propensity、迭代缩短；30 副本只适合查管线 | 已改 | 每个副本在重采样上重新拟合 propensity（`--ipw` 时）并用完整迭代数（propensity 迭代也不再减半）；`--boot` 默认 200（`DEFAULT_BOOT`），offline_v2 `--boot` 默认 200；<200 时报告写"pipeline check"。测试里用小副本数 |
| 2.11 | 共享主干 + 处理交互 logistic 或带收缩的分处理模型 | 已有 | 现有 treatment 模型就是共享 base + 选项交互 + L2 收缩，未改 |
| 2.12 | 交叉拟合 DR（含公式，写清依赖条件） | 已改（可选） | `gui-gate train --model treatment --dr`：5 折按会话交叉拟合，φ 公式、截断计数、会话自助法 CI、四条假设写进报告 JSON 和文档 §10.5；测试 `task_label_is_refused_and_dr_recovers_a_known_effect`（模拟数据 τ=0.3 被恢复） |
| 2.13 | check：没有 probe 模型时翻转特征倒推 P(pass) | 已改 | `check_pass_prob` 无 probe 返回 None；legacy 的 voi 分支不再倒推（日志写 `CHECK_HEURISTIC_NOTE`，走灰区启发式；默认 check_model=legacy 不经过这里，golden 不变）；treatment 下无 probe 模型时 check 无估计依据 |
| 2.14 | check 选择受与其他选项相同的区间约束；叫"便宜验证启发式"；重绑参数时包含修复效果 | 已改 | 有 probe 模型时按副本算 VOI 区间，进同一个下界规则；chain 里 check 的 `rebound/includes_repair`，事件 `rebound` |
| 2.15 | C_fail 是先验 | 已改（文档） | `decide::fail_cost` 注释 + 文档 §9；value 日志（treatment）`fail_cost_kind` |
| 2.16 | λ 与 η 分开；5–10% 介入预算由 η 控制 | 已改 | `GateConfig.gamma`（`GUI_GATE_GAMMA`，默认 1）、`eta`（`GUI_GATE_ETA`，默认 0），只作用于 treatment 路径；默认路径不变 |
| 2.17 | 人工复核 click/scroll 失败标签（scroll 35 个失败中 23 个来自重复判定） | 代码已改，**待用户** | offline_v2 `--scroll-repeat failure|not_failure`，报告同时给两种口径（`label_sensitivity`）；人工复核需要用户看日志 |
| 2.18 | 新增特征按组消融（可视区/enabled/遮挡/坐标系/应用类型），报配对 AUC/Brier 区间 | **未做** | 这些量不在任何已有日志里（审核也说历史日志补不出来）；需要驱动先上报再采新日志，属于桌面版新日志采集工作 |
| 2.19 | 坐标偏移记录坐标系、窗口原点、DPI、实际偏差 | **未做** | 同上，需要驱动帧信息；本分支没有加任何 27 px 之类的补丁 |
| 2.20 | v2 特征 AUC 增量无可靠收益 | 已有 | 本次重算：−0.0029 [−0.017, +0.013]（见数字） |

## 第 6 节 离线与在线评测

| # | 审核意见 | 状态 | 做了什么 / 原因 |
|---|---|---|---|
| 6.1 | 结果口径：185/361=51.25%、185/332=55.72%、28 infra、1 未运行、非官方环境；53.7% 已过时 | 已改 | docs/value-model-v2.md 开头、CHANGELOG；README 和 docs/failure-analysis-v099.md 在 acaba69 已是新口径；results/README.md 里 53.7% 只作为"审核引用的旧快照"出现，保留。**gui-pi-desktop 的设计文档里的 53.7% 在另一个仓库，本分支没改** |
| 6.2 | GUI 与命令分开 | 已有 | |
| 6.3 | 重训是同一运行内留一领域，不是跨 run / 跨任务模板 | 已改 | 报告 `generalisation`：跨任务族（留一领域）、跨任务（开发组→测试组）分开；跨 run 写"not available: one run" |
| 6.4 | 同一任务及布局变体同组 | 已改 | `task_group`（`--variants` 映射，或去掉 `-v<N>`/`-layout<N>` 后缀）；自助法和划分都按组 |
| 6.5 | 模型/校准器/阈值/预算价格只在开发集选；去掉在测试集上试 L2 挑最好 | 已改 | `select_l2_on_dev`（开发组内 4 折，按 NLL）；测试组只评一次；0.10.2 文档里"弱 L2 得 AUC 0.795"是在留出集上试出来的，已在文档 §5.3 撤回并标为事后结果 |
| 6.6 | 基线：vp_direct、固定观察、固定推理、风险阈值、启发式、同预算随机、等时等待 | 已改（离线能算的已算） | 离线：同预算随机、启发式（与 value.rs heuristic 相同规则）的筛选表；其余需要环境，报告列为 online-only（策略试点三个条件 + 其余作为后续臂） |
| 6.7 | 预算筛选只是事后 top-k，不能写成"介入 5% 能避免 43.1% 失败" | 已改 | 报告标题和说明改为"post-hoc top-k screen"，claims 增加 `flagging_budget: post_hoc_top_k_screen`；文档 §5.2 同步 |
| 6.8 | 成本多由下一动作间隔估算、剔除>600 s；补组件计时、超时记录、尾部分位 | 已改 | 新日志有组件计时（chain、exec_t0_ms、continues），`gui-gate costs` 优先用组件计时，换动作/超时记删失；报告中位数/p90/p99/删失数；旧数据标 `next_action_gap_upper_bound`（上界/相关代理） |
| 6.9 | 最小在线方案：策略试点 + 增量试点 | 计划已生成，**待在线验证** | `bench/online_plan.py pilots`：策略试点（Chrome/Impress/Calc/multi_apps 各 3 题，种子 20261026，direct-with-safety/legacy/frozen-new，seed 0/1/2，108 次）；增量试点（24 个可恢复检查点，direct/look_full/匹配等待 ×3，216 分支，按检查点聚类）。示例输出 `results/online-plan-v3-2026-10-26/`（status=draft，因为只给了 gate_commit） |
| 6.10 | 重复先任务内汇总再配对 bootstrap/置换检验，不对重复平均用 McNemar；分支按检查点聚类 | 已改 | 两个试点和旧计划的 analysis 节；旧计划只在 repeats==1 时允许 McNemar |
| 6.11 | 冻结 Pi、主模型及 thinking、审核模型、角色、提示词、驱动、应用环境、超时、成本配置 | 已改 | `FREEZE_KEYS`（12 项，含 gate_commit、value_weights），缺任何一项只能生成 draft，`--require-frozen` 直接拒绝；测试 `test_pilots_refuse_frozen_without_every_hash` |
| 6.12 | infra：主分析保留全部计划单元，有限补跑，另报有效配对，算法挂起不算 infra | 已改 | `INFRA_RULE`（旧计划不再"双臂排除"） |

## 第 8 节第 2、3 项

| # | 完成标准 | 状态 |
|---|---|---|
| 8.2 | 不可变处理前特征、明确处理链与续接关系；执行/进展/任务标签分离；命令单独建模；未知覆盖明确标出 | 已改（命令单独建模在 v2 已有） |
| 8.3 | 冻结小策略做配对分支与在线对照，同时报增量校准、任务成败、总耗时和介入预算；失败和 infra 固定口径；不把离线筛选当在线提升 | 计划和分析代码已就绪，**待在线验证**（未运行，按要求不做在线测试） |

## 另：gate_next_step（第 5 节，本分支任务第 8 项）

已改：不再用虚构 token click 调 `choose()`；改为独立规则 `research::next_step_rule`，与自动观察的重复读取判断共用
`research::auto_record_fresh`（刚随动作结果附过树就返回 none）；固定基线策略（direct/light/full、observe_*）照旧生效；
返回里加 `rule: "freshness"` 和 escalation 建议；字段 observe/extra_reason/reason/decision_id 与桌面版 0.3.1 使用的一致。

## 数字（`results/offline-v3-2026-10-26/`，seed 0，v0.9.9 运行，只读输入）

`python3 bench/offline_v2.py results/osworld-verified-v099-ds41-2026-10-23 -o results/offline-v3-2026-10-26 --bin <gui-gate> --boot 200`
（v3-res 构建）。审计口径与报告一致：185/361 = 51.25 %，185/332 = 55.72 %。

- **处理前状态无法恢复**（旧日志无 x_pre，trail 跑过 wait/check/heal）：**388 行**排除出因果估计（GUI、非命令、332 个日志的首会话）；
  这些日志没有一行带 x_pre（新字段从本版开始记录）。
- **hand-back 归属**：同一候选（同签名）可归属 **24** 个，换了动作或超时"无法归属" **133** 个（不给标签）。
- 排除后 treatment 训练行：direct 6,863、look_full 16、review 20；wait / check / heal 历史行全部因处理后特征被排除。
- **状态级覆盖**（训练状态中 e_o 与 e_direct 都 ≥ 0.05 的比例）：look_full 0.0 %，review 0.18 %；IPW 计数后只有 direct 有依据。
  也就是说，按新的覆盖规则，现有日志对任何介入选项都**没有估计依据**，treatment 模型在运行时会一律回退旧先验并写 `fallback: legacy_heuristic`。
- treatment v2（family 留出，200 次自助重拟合，每次重估 propensity）：IPW AUC 0.708 [0.650, 0.752]，ECE 0.030；不加权 0.738 [0.677, 0.783]。
- GUI 预测器：L2 在开发组上选为 0.1（网格 0.1/1/10 的最低端，下一版可加 0.01 再选）；留一领域 v1 重训 AUC 0.775 [0.745, 0.803]、v2 0.772；
  跨任务（196 开发组 → 85 测试组，2,460 行）v1 0.799 [0.756, 0.847]、v2 0.782 [0.736, 0.835]；v2 − v1 = −0.0029 [−0.017, +0.013]；跨 run：只有一次运行，不可用。
- 事后 top-k 筛选（5 %，跨任务测试组）：v2 精度 0.585 / 召回 0.411；同预算随机 0.069 / 0.052，启发式 0.089 / 0.066（全体行）。只是筛选，不是避免失败。
- scroll 重复标签两种口径：失败 483 / 460（scroll 失败 35 / 12，23 个来自重复判定），人工复核待用户。
- 成本（旧日志，下一动作间隔，上界）：look_full 中位 9.8 s、p90 62.8 s、p99 217 s；review（拒绝）中位 7.1 s、p90 66 s；ask_human 中位 32 s；>600 s 删失 0。
- 增量校准：no paired branch data（空）。

## 可能与 v3-sec 冲突的文件

- `src/server.rs`：`Inner` 末尾加 `research` 字段及初始化；`gated()` 里加 3 行（执行计数比较 + `research.finish`）；
  `gated_inner` 中：x_pre 捕获、check/wait/heal 三个分支的计时、事件构造后一个集中块、执行成功处 2 行标签；`next_step` 整体重写；
  `duplicate_read` 一行改为共用函数。未碰派发检查、unknown_sigs、审批/permit、问人路径。
- `src/decide.rs`：`GateConfig` 加 `gamma`、`eta` 两个字段及环境变量；`fail_cost` 注释。
- 新文件 `src/research.rs`，`src/main.rs` 加一行 `mod research;`。
