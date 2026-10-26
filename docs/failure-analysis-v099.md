# 失败归类：OSWorld-Verified v0.9.9 全量运行（gui-pi3）

数据：`results/osworld-verified-v099-ds41-2026-10-23/`（提交 9450527，定稿 361/361），离线评估 `results/offline-v2-2026-10-26/`。
脚本：`python3 bench/failure_analysis.py results/osworld-verified-v099-ds41-2026-10-23 --osworld <OSWorld checkout> -o ...`，
输出在 `results/failure-analysis-v099-2026-10-26/`（`summary.json` 汇总，`failures.json` 每个失败一行，含信号与标记）。
只读原始结果；选取规则与 bench/offline_v2.py 相同（`first_valid`：每题第一次非 infra 的尝试）。

> 口径：这是基于 OSWorld 任务与评测器的**自定义环境**结果（Debian/XFCE、Chromium、无 VM 回滚、--human deny），
> 不是 OSWorld 官方成绩，不能与排行榜比较。有效 185/332 = 55.72 %，计划 185/361 = 51.25 %，28 个 infra 不在本文。

## 1. 总表（147 个有效失败，每个失败只归一个主因，按下列优先级）

| # | 主因 | 数量 | 日志里能判断的依据 |
|---|---|---|---|
| 1 | 预算截断（timeout 3600 s / 100 轮） | 20 | `stop` |
|   | ├ 其中：模型调用挂起（中转站无响应，最后一个 Pi 事件是 turn_start/message_start，之后 ≥600 s 无事件） | 5 | Pi `.times` |
|   | ├ 其中：工具调用挂起（check_done 1938 s 无返回） | 1 | Pi `.times` + 门日志 |
|   | ├ 其中：执行错误多（≥5 个已执行动作报错） | 3 | 门日志 |
|   | └ 其中：进展慢（正常推进但用完时间/轮数） | 11 | |
| 2 | 门拦截后放弃（门拒绝/要求人确认，最终答复 FAIL 或点名门） | 11 | 门日志 + 最终答复 |
| 3 | 网站封锁环境（403 / bot check / Akamai，改用别的网站） | 6 | 最终答复 |
| 4 | 不可行任务未识别（OSWorld `infeasible` 任务答了 DONE） | 3 | 任务配置 |
| 5 | 可行任务答 FAIL（未点名门） | 3 | 最终答复 |
| 6 | 循环（loop_breaker ≥3 或重复 ≥6） | 3 | 门日志 |
| 7 | 门放行后执行失败（≥5 个已执行动作报错） | 5 | 门日志 |
| 8 | 答 DONE 但结果不对：模型能力 / 任务理解 / 评测器严格度（日志无法再分） | 96 | 其余 |

按领域：chrome 31、gimp 10、calc 6、impress 35、writer 5、multi_apps 37、os 9、thunderbird 3、vlc 7、vs_code 4。
不可行任务：已知 26 个，成功 21 个。

### 1.1 「门拦截后放弃」11 个逐个人工判读

| 任务 | 门做了什么 | 判读 |
|---|---|---|
| chrome-35253b65 | 点击菜单项 "Cast, save, and share" 被当成「分享」，审核拒绝、要求人批准（human=deny） | **门误拦**：该行只提供 `open` 动作，是子菜单入口，点击不分享任何东西 → 已修（§2.2） |
| libreoffice_impress-bf4e9888 | 只读脚本里的 `str.replace` 被当成移动文件拒绝 2 次；随后覆盖任务要求的 pptx 需要人批准 | 前者**门误拦**（已修，§2.1）；后者是按设计请求确认，human=deny 下无法完成 |
| os-a4d98375 | `str.replace` 误拦 + sudo 需人批准 | 主因是环境差异（XFCE 没装锁屏程序）；`str.replace` 误拦已修 |
| chrome-44ee5668、chrome-7b6c7e24 | 删除历史 / 站点数据（任务本身要求）需人批准 | 按设计，human=deny |
| multi_apps-869de13e、multi_apps-ce2b64a2 | `mv` 移走 / 重命名已有文件需人批准 | 按设计，human=deny |
| os-5812b315、os-94d95f96、os-b6781586 | `sudo` 需人批准 | 按设计，human=deny（OSWorld VM 有 sudo 密码，本环境的协议不同） |
| thunderbird-15c3b339 | 输入密码字段交给人（handoff） | 按设计，human=deny |

即：门本身的缺陷 1 个主因 + 2 个参与；9 个是「按设计要人确认，而评测用 human=deny 无人值守」——这是评测协议的代价，
桌面版有真人回答，不属于门缺陷；1 个是环境差异。

### 1.2 贯穿失败的标记（一个失败可有多个；同时给出成功运行里的出现次数，避免把常见现象当成失败原因）

| 标记 | 失败运行数（次数） | 成功运行数 | 读法 |
|---|---|---|---|
| 门要求先观察（look_full / look_light） | 53（73） | 52 | 成败两边一样多，没有证据是失败原因 |
| `rm` / `>` 碰到 /tmp 或工作目录里的旧文件（多为**前面任务**留下的 chk/、gimp.log 等，测试机不回滚） | 30（37） | 46 | 环境残留；成功运行里更多。是否把系统临时目录或「本用户以前的 gate 会话创建的文件」放行属于策略变化，列入待研究 |
| 审核模型拒绝 | 27（30） | — | 审核共 125 次：81 拒 / 44 放；多为上面的规则误报或任务确需的写操作 |
| `str.replace` 被当成移动文件 | 17（25） | 7 | 门缺陷，已修 |
| 最终答复提到环境差异（Chromium、版本、未安装） | 24 | — | 环境 |
| 部分得分（0 < score < 1） | 8 | — | 评测器按比例给分的任务 |
| 验证码 handoff | 3（5） | — | 网站环境 |

## 2. 本次修复的门缺陷（只修有日志证据、且是通用缺陷的；每项有回归测试）

### 2.1 Python/JS 的字符串方法 `x.replace(a, b)` 被当成「脚本删除或移动文件」
- 证据：全部门日志里 40 次拒绝、29 个日志（被采用的运行中：失败运行 17 个共 25 次，成功运行 7 个），原因都是
  `script code deletes or moves files (.replace(')`，例如只读检查脚本 `print(d[:600].replace('\n',' '))`
  （libreoffice_calc-42e0a640）。每次都要再经审核模型，审核多半拒绝。
- 原因：`command.rs` 的 `code_delete` 规则把任何 `.replace('` 都当成 `pathlib.Path.replace(target)`。
- 修复：`.replace(` 形式（不是 `os.replace`）先数调用的顶层参数（跳过引号与嵌套括号）；≥2 个参数就是字符串方法，
  不算文件操作。`os.replace`、`Path.replace('x')`（一个参数）、`shutil.move`、`os.remove` 等照旧；调用不闭合时按旧规则（保守）。
- 回归测试：`command::tests::string_replace_is_not_a_file_move`。对这 40 条原始命令离线重算：39 条不再标记，
  剩 1 条命令里真的有 `os.unlink(`，仍正确标记。

### 2.2 只有 `open` 动作的菜单项（子菜单入口）按标签词判风险
- 证据：chrome-35253b65（任务：建桌面快捷方式）。Chromium 菜单行 `menu item "Cast, save, and share" [actions=[open,showContextMenu]]`
  因含 "share" 被判中风险 → 审核拒绝 → 要求人批准（human=deny）→ 任务失败。同一棵树里叶子命令是 `[select,...]`，
  子菜单入口（如 "More tools"）是 `[open,...]`。
- 修复：`Element` 记录行的无障碍动作列表（树 markdown 的 `[actions=[...]]` 或 elements JSON 的 `actions`）；
  角色是菜单项、动作含 `open`/`expand` 且不含 select/click/press/activate/invoke/doDefault/toggle/confirm 时，
  点击它与打开菜单同等对待（只显示选项，不改变任何东西），不再按标签词判风险。没有动作列表时不变（保守）。
- 回归测试：`features::tests::submenu_parent_is_not_risky_but_leaf_items_are`。

两项都不改价值模型（默认仍 `legacy`），不改阈值，也不针对某道题。

## 3. 桌面版待研究（可能过拟合到这 361 题或属于策略变化，本版一律不做）

1. **临时文件归属**：测试机不回滚，前面任务留在 /tmp 的 chk/、*.log 让后面任务的 `rm -rf /tmp/chk`、`> /tmp/gimp.log`
   都要人批准（37 次）。是否放行系统临时目录、或记录「本用户以前 gate 会话创建的文件」要在真实桌面上研究，
   而不是为基准环境放宽。
2. **`.rename(` 同类误报**（pandas `df.rename(columns=...)`）：机理与 2.1 相同，但本次日志里没有实例，未改。
3. **check_done 没有总时限**：1 例（multi_apps-deec51c9 第 3 次尝试）check_done 1938 s 无返回、无日志，直到 3600 s 超时。
   每个子步骤都有时限（命令 15 s、文档读取 30 s、模型请求 120 s），日志定位不到挂在哪；需要整体截止时间和更细的日志。
4. **模型调用挂起**：5 个超时是中转站不回包（Pi 没有单次请求时限）。桌面版应给主模型每次请求设时限并重试；
   bench 应把它记为 infra。
5. **审核模型拒绝率**：125 次审核 81 次拒绝，多数是任务确需的写操作；调审核提示/阈值容易拟合到这批题，需在桌面版用新任务集评估。
6. **人确认的代价**：9 个失败是 human=deny 下必须有人回答的操作（sudo、删除、覆盖、密码）；桌面版有真人，
   应统计真人在这些请求上的负担，而不是放宽规则。
7. **96 个「DONE 但不对」**：日志无法区分模型能力、任务理解与评测器严格度，需要逐题对照评测器（含 Chromium vs Chrome 等环境差异 24 例），
   属于模型/环境研究，不是门的缺陷。
8. **价值模型**：离线 v2 结论不变（命令导致旧预测器失准；v2 点击/滚动特征无 AUC 提升；「避免了失败」需在线反事实方案
   bench/online_plan.py，未运行）。默认仍 legacy。
