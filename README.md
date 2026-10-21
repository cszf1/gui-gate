# gui-gate

**Cua Driver + 点击前价值门，合成一个 MCP server。** 任何支持 MCP 的 Agent（Pi、opencode、Claude Code、Codex……）加一行配置就能用。

```
Agent (Pi 等) ──MCP stdio──▶ gui-gate ──▶ cua-driver（手脚：AT-SPI / UIA / AX + 输入）
                              │
                              └─ 每个动作执行前：值不值得先看一眼 / 多想一下 / 等一等？
```

Cua Driver 的 66 个工具原样暴露。观察类工具（`get_window_state` 等）直接放行并被门记录；
动作类工具（`click`、`type_text`、`run_steps` ……）先过门。

## 门的三层判断

| 层 | 做什么 | 代价 |
|---|---|---|
| 0 硬规则 | 高风险（删除/发送/支付…）要用户确认；旧快照的元素 token 自动修复；同一动作刚失败不许盲目重发；新弹窗/新窗口出现要先看；像素点击但窗口变了要截图 | 0 |
| 1 轻量预测器 | 用运行时特征算 P(直接执行成功)，比较各选项的 `λ·代价 + (1−P)·失败代价`，取最小 | <1 ms |
| 2 灰区探测 | 拿不准时，先花 ~30 ms 只读界面树（不截图）把 token 重新绑定到同名同角色元素；仍不行才让模型重看 | ~30 ms |

选项：`direct` 直接执行 · `check` 便宜探测 · `heal` 修复过期 token · `wait` 等界面稳定 ·
`look_light` 让 Agent 只读树 · `look_full` 让 Agent 截图重看 · `reason` 让 Agent 复核目标 · `ask_human` 请用户确认。

门拦下时返回以 `[gate]` 开头的一句话，告诉模型具体怎么做，**不会执行任何动作**。

特征全部来自运行时（观察多久前、之后做了几步、token 是否过期、上一步是否报错/弹窗、是否重复、
标签是否重名、界面是否在加载、窗口是否变化、像素点是否贴边……），不使用任何生成器真值。

## v0.4 新增（借鉴 Claude Code auto mode / Codex auto-review，见 docs/）

- **看用户原话授权**：`--task` / `--task-file` / 环境变量 `GUI_GATE_TASK`，或 Agent 开头调用一次 `gate_task`
  （原文照抄，只能设一次）。原话明确说了「覆盖 / 删除 / 发送……」且对象对得上，风险步骤直接放行；
  覆盖当前正在编辑的文档本身也放行。
- **拦了接着干**：被拒时返回理由 + 安全做法（换名保存、跳过冲突文件、先列出再执行……），
  同一后果换条路做也会被拒；连续 3 次 / 累计 8 次被拒才让 Agent 停下汇报。
- **独立审核员（可选）**：设置 `GUI_GATE_REVIEWER_URL/_MODEL/_KEY`（OpenAI 兼容接口）后，
  中风险步骤由一个只看「用户原话 + 具体动作 + 对话框文字」的小模型判，不再让干活的模型自己复核。
- **省观察**：截图只在刚用过坐标点击时附带；动作后紧接着重读同一窗口直接用缓存（`fresh:true` 强制重读）；
  表格里「粘贴覆盖单元格」不再当成覆盖文件。
- **服务端批量执行（任何 Agent 可用）**：`run_actions` 一次传多步，每步单独过门，被拒 / 报错 /
  意外弹窗 / 期望不符就停，整批结束只观察一次；坐标点击只允许第一步。
  `gui_script`（`pip install .[codemode]` 装 quickjs 后出现）写 JS 脚本，用法同 Pi codemode
  （`await tools.click({...})`、`text()`、`return`、`store/load`），在子进程沙箱里跑、超时即杀。
  `--batch auto|actions|off`。

## 安装与接入 Pi

```bash
curl -fsSL https://cua.ai/driver/install.sh | bash      # Cua Driver（Linux 还需 at-spi2-core）
pip install .                                           # 本仓库，零依赖
pi mcp add gui -- gui-gate                              # 接入 Pi
pi mcp list                                             # 检查连接
```

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

## 采数据 → 训练 → 对比

```bash
gui-gate --policy direct --collect --log runs/s1.jsonl     # 不干预地采集，标签无偏
gui-gate-train runs/ -o weights.json                       # 按会话留出、Platt 校准，输出 ECE/Brier/AUC
gui-gate-compare runs/                                     # 各策略：执行数、失败率、观察/截图数、门开销
```

日志默认写到 `~/.gui-gate/logs/`，每行一个事件（JSONL）。

## 试验台

`scripts/fixture_app.py`：GTK3 测试程序，带改名、加载、弹窗三种真实干扰，真值单独记录用于评分。
`scripts/sim_agent.py`：**脚本化**的 Agent（不是大模型），用来在真实桌面上把整条链路跑通。
`scripts/run_matrix.sh`：同一批种子在各策略下跑，加上采数据和训练。

## 测试

```bash
python3 -m unittest discover -s tests -v     # 用假 driver 走真实 MCP stdio
```

## 许可

MIT。Cua Driver 为 trycua/cua 的 MIT 项目，本项目只通过其公开的 MCP 接口调用它。
