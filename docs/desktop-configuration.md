# 桌面配置管理

入口：主导航「桌面配置」。运行时为现有 Tauri / Rust，不要求用户安装 Node.js 或 npm 版 Codex。

## 配置和代理同时启动

「桌面配置 → 启动此配置」会将已核对的配置目录作为子进程的 CODEX_HOME；默认勾选「使用已保存的代理」，同时传入代理启动器中已保存的 HTTP_PROXY / HTTPS_PROXY / ALL_PROXY / NO_PROXY。两者是独立选项，不是二选一，不会删除或改写原有指令或外部 codex-purge 配置。

启动必须再次确认，后端在关闭旧会话之前重新核对安装、配置版本和代理设置。未保存的编辑会阻止启动。取消确认不会重启 Codex。取消勾选仅移除子进程中的 Guard 代理变量，不改变系统代理。启动回执分别记录配置目录和是否注入代理，不能把未启用代理的会话显示为代理启动。

仅「保存配置」仍不会重启；只有明确确认「启动此配置」才会替换已验证的现有会话。外部受管配置虽禁止写入，但允许按原样启动并同时使用代理。

本地重新打包可运行 `npm run package:local`：输出独立的 EXE、NSIS、MSI、便携 ZIP 与校验文件，位于 release-artifacts/local-<版本>-<时间>。本地包不生成自动更新签名，也不修改正式发布的公钥配置。

## 使用范围

- 自动复用代理启动器的桌面安装发现和手动路径选择，定位同一安装的 `resources/codex.exe`。
- 配置目录按手动选择、Guard 继承的 `CODEX_HOME`、当前用户 `.codex` 依次确定。界面明确显示来源，**不声称它就是运行中桌面的实际数据目录**。
- 仅编辑用户配置中的 `developer_instructions`、`model_reasoning_effort`、`approval_policy`、`sandbox_mode`。未明确选择的项、项目文件、提供商凭据和其他表保持不变。
- 同一配置目录可能供 CLI 和桌面端共用。不会创建或接管全局 shim，不修改 WindowsApps、app.asar、Codex 二进制或项目 AGENTS.md。
- 检测到 codex-purge 的受管配置块时禁止接管写入；需先由原工具恢复。
- 不自动停止或重启桌面应用。保存后可在方便时重开 Codex 并创建新会话。云端编排、会话覆盖、项目覆盖与组织要求仍需单独确认。

## 状态证据

1. **实例**：使用当前 Windows 会话的已验证进程路径和桌面安装定位，不按进程名认定同一实例。不能读取的进程单独计数。
2. **文件**：区分缺失、格式错误、无法读取、外部受管；不把错误当成健康。
3. **引擎兼容性**：由选中安装的内置 CLI，在临时 CODEX_HOME 中执行 `initialize` / `config/read`。只复制四个允许的配置项（高级值也参与类型检查），字符串值逐项读回比较；不复制其他配置项、凭据、登录、MCP、插件、提供商或其他用户目录内容。清空继承环境中的 API 密钥与代理。
4. **会话**：检测结果始终包含 `active_session_verified: false`。不发起 `thread/start` / `turn/start`，不以「配置可读取」推断模型请求、当前桌面会话或云端任务已生效。项目层和会话层覆盖不在此用户配置兼容性断言范围内。

版本探测最多 5 秒，配置 RPC 最多 15 秒。限制输出大小；Windows 检测进程置于 kill-on-close Job Object 中，完成或超时后关闭。此过程不调用模型。

## 写入与恢复

应用和恢复均需独立预览与确认。后端重新确认目标 CLI 身份（路径、大小、修改时间）和原文件 SHA-256，兼容性检查后再次验证目标，并在最终提交前重新检查文件版本。

`%LOCALAPPDATA%\CodexBootGuard\desktop-config\<目录标识>\<操作编号>\` 下保存：

- `entry.json`：操作类型、目标目录、变更键、前后哈希、`prepared` / `committed` 阶段。
- `before.toml` / `after.toml`：存在时保存原始字节；文件缺失与空文件分别记录。

先保存并核验快照，再以同目录临时文件原子替换配置。Windows 使用 ReplaceFile 保留原目标的 DACL，文件占用时有限重试。跨 Guard 进程使用独立配置操作锁。预览与写入间文件发生变化则拒绝，不覆盖并发编辑；非协作外部编辑器在最终检查与系统替换之间仍存在极小的竞态窗口。

- 当前文件未变化：字节级恢复，包含原 BOM、CRLF 和格式。
- 当前仅有无关修改：核对本次拥有的配置值后，仅恢复这些键，保留其他表和新增内容。
- 同一配置项有冲突：拒绝恢复，保留当前文件与全部快照，不静默覆盖。
- 重复应用相同配置：不产生新快照、不覆盖所有权记录。
- 写入后记录提交中断：保留 prepared 快照，刷新后可核对并恢复。
- 恢复本身也建立新快照，因此可再恢复。历史显示最近 30 项，不自动删除旧快照。
- 路径中的符号链接、junction/reparse point、越界编号、损坏或哈希不匹配的快照会被拒绝。

备份可能含原文件中的敏感信息，仅留在当前用户本机数据目录，不自动上传或导出。

## 测试

```powershell
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run test:ui

# 显式真实引擎与完整事务测试；用户配置及 Guard 数据均隔离。
$env:GUARD_TEST_CODEX_CLI = (Get-Command codex.exe).Source
cargo test --manifest-path src-tauri/Cargo.toml --lib desktop:: -- --include-ignored --nocapture
Remove-Item Env:GUARD_TEST_CODEX_CLI

npm run build:desktop:debug
npm run test:desktop:native
npm run test:native
```

原生 UI 测试采用临时 CODEX_HOME、WebView2 用户目录与 CDP 端口 19329，验证真实 Tauri IPC、兼容性、取消预览和拒绝路径，不执行确认写入；完整保存/重复应用/并发编辑/恢复由 Rust 的真实桌面事务测试验证。新测试的截图和结果在系统临时目录，不包含真实用户配置。现有原生测试继续使用独立端口 19327。

## 参考与实现取舍

- [codex-purge](https://github.com/bandageok/codex-purge/tree/294f8f9d3dcbee54717dea8cb0eebf25a2b22be1)：配置层思路；未复制默认指令、shim 或二进制修改逻辑。
- [mmparko/dsh-purge](https://github.com/mmparko/dsh-purge/tree/033e6bd1e0f101352b39075ecd0d758ffef26b76)：桌面实例和资源定位的分层思路。
- [DSH 社区版](https://github.com/kingcharleslzy-ai/dsh-purge/tree/126caa2c99297223a7e0e87e604aecfc802f396e)：区分磁盘与运行证据、当前实例范围、事务快照和恢复验证的设计。
- [OpenAI app-server 文档](https://learn.chatgpt.com/docs/app-server)：公开配置 RPC。

本模块为 Rust / React 原生实现，没有直接嵌入上述第三方源码、提示词、补丁或运行时。
