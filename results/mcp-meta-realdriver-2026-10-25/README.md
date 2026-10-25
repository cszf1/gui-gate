# 真驱动协议测试：MCP 2026-07-28 `_meta`（2026-10-25）

环境：本地沙箱 Linux aarch64，Xvfb :99（1280x800）。驱动：真 cua-driver 0.38.0
（trycua/cua 发布 `cua-driver-rs-v0.38.0`，资产 `cua-driver-rs-0.38.0-linux-arm64-binary.tar.gz`，
sha256 80c9ae66…b33d，已按同一发布的 SHA256SUMS 校验通过）。

gui-gate：v0.9.9 = 官方发布资产 `gui-gate-linux-aarch64.tar.gz`（SHA256SUMS 校验通过）；
main = 433990c 本地 `cargo build --release`。评审模型配置是假的本地地址（127.0.0.1:9），只调只读工具，评审不会被调用。

客户端：`realdriver_client.py`
- modern：不发 initialize；`server/discover`、`tools/list`、`tools/call` 都带 `params._meta`（protocolVersion 2026-07-28 + clientCapabilities + clientInfo）
- legacy：`initialize`（2025-06-18）+ `notifications/initialized` + `tools/list` + `tools/call`
- 调用的工具：`list_apps`（门的 core 工具集里没有 get_screen_size）

| gui-gate | modern 客户端 | legacy 客户端 |
|---|---|---|
| v0.9.9 | **失败**：tools/call 返回 isError "Missing required per-request MCP metadata"；server/discover 原样转发驱动的（serverInfo cua-driver，没有门的说明） | 成功 |
| main 433990c | **成功**：tools/call 正常（resultType complete，serverInfo gui-gate）；server/discover 由门回答，含 [gui-gate] 说明，supportedVersions ["2026-07-28"] | 成功（initialize 由门回答，含门说明） |

结论：真驱动的行为和 fake driver 一致，修复有效，没有发现需要改的代码。

文件：`099-modern.json` `099-legacy.json` `main-modern.json` `main-legacy.json`（完整请求/响应；图片 base64 去掉，
list_apps 的进程列表已删，只留第一行）。

复现：`DISPLAY=:99 python3 realdriver_client.py <gui-gate> <cua-driver> out.json modern|legacy`
