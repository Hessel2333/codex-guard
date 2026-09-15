# Codex Guard

Windows 10 / Windows 11 的 Codex 启动健康检测与代理启动工具。

应用名为 **Codex Guard**，npm/Cargo 包名及 EXE 文件名统一为 `codex-guard`。应用标识 `com.codexbootguard.desktop` 和 `%LOCALAPPDATA%\CodexBootGuard` 数据目录保留，以兼容已有配置与日志。

技术栈：Tauri 2、React、TypeScript、Vite、Rust。当前版本 **0.4.1**。

## 已实现

### 启动健康检测

- 原生 Windows PackageManager 查询当前用户 OpenAI.Codex，按四段数字版本选择最新 main package。
- 查看安装位置、PackageFullName、PackageFamilyName、bundled CLI 大小/修改时间/可读性及可获取的文件版本。
- 从 HKCU 持久化用户环境读取 CODEX_CLI_PATH，并与当前版本 bundled CLI 路径比较。
- Overview、Diagnostics、Refresh、浅色/深色/跟随系统设置。

### 一键修复启动路径（0.3.0）

运行概览提示“建议修复启动路径”时，点击“一键修复路径”，确认旧路径和目标路径后，将当前用户 `HKCU\Environment\CODEX_CLI_PATH` 持久化为最新已安装 Codex 的内置 CLI 路径。无需管理员权限。

后端重新检测安装包与文件，并拒绝过期确认、缺失或不可读的 CLI，以及无法读取的环境变量。写入后读回核验、通知 Windows 环境变更，再重新检测健康状态。写入失败或后续核验失败会显示实际错误。

修复不会关闭 Codex；已运行的 Codex 和终端需重新打开，必要时注销后登录。路径已经正确时无需修复。代理启动仍可为新进程单独提供正确路径。

### Proxy Launcher

已把 `D:\Codes\codex\_proxy` 原有脚本功能移入 Tauri 应用的 **Proxy Launcher** 页面，使用 Rust / Windows 原生 API 实现。原目录保持不变，运行新应用不依赖该目录或 PowerShell 脚本。

| 原功能 | Tauri 中的入口 |
| --- | --- |
| 普通代理启动 start | Launch with proxy |
| 管理员代理启动 | Administrator launch，确认后由 Windows 请求 UAC |
| restart | Restart，关闭已验证会话后以代理配置重新启动 |
| stop | Stop Codex，确认后先请求正常关闭，再结束仍存活的目标进程 |
| status | Processes，显示 PID、路径、启动时间及是否归本应用管理 |
| log | Proxy logs，实时查看新日志或 Original launcher 原日志 |
| 创建/刷新快捷方式 | Create / refresh shortcuts，普通 P 图标及管理员 A 图标 |
| CODEX_HTTP_PROXY / CODEX_ALL_PROXY / CODEX_NO_PROXY | 可编辑并保存的代理设置 |
| CODEX_APP | Desktop executable override |

默认配置与旧脚本一致：

```text
HTTP_PROXY / HTTPS_PROXY = http://127.0.0.1:7890
ALL_PROXY                = socks5://127.0.0.1:7890
NO_PROXY                 = localhost,127.0.0.1,::1
```

首次没有保存配置时，读取启动 Boot Guard 进程继承的 CODEX_HTTP_PROXY、CODEX_ALL_PROXY、CODEX_NO_PROXY、CODEX_APP；保存后以应用配置为准。

代理变量通过 Rust Command 的子进程环境传入，Windows 环境变量名不区分大小写。不修改系统代理、不启用 TUN、不写全局 HTTP_PROXY。若桌面程序旁的 resources/codex.exe 存在，也会把这一 CLI 路径传给新进程，避免继承旧路径；代理启动本身不会改用户 CODEX_CLI_PATH，持久化修改由运行概览中的一键修复完成。

启动和重启都可能关闭现有 Codex，会先展示确认框。原生后端也拒绝没有确认的停止/启动请求。只管理当前 Windows 会话中，路径匹配选定桌面 EXE 或其 resources 子目录的相关进程；无法验证的进程不会按名称强制结束。退出前再次验证进程路径及创建时间，防止 PID 被复用。

管理员启动仅提升同一 EXE 的短生命周期 helper，主界面保留原权限；helper 在创建 WebView 前处理请求。UAC 必须使用同一个 Windows 账户，取消/失败会返回实际错误。快捷方式先打开相应启动确认框，再执行；原脚本创建的快捷方式不被替换。

## 数据和日志

```text
%LOCALAPPDATA%\CodexBootGuard\proxy-settings.json
%LOCALAPPDATA%\CodexBootGuard\proxy.log
%LOCALAPPDATA%\CodexBootGuard\proxy.previous.log
%LOCALAPPDATA%\CodexBootGuard\proxy-last-launch.json
```

旧脚本日志只读显示：`%LOCALAPPDATA%\codex-proxy\codex-proxy.log`。

日志每次读取最近 64 KiB；新日志超过 512 KiB 时轮转。应用操作使用跨进程文件锁，JSON 先写临时文件再替换。新日志不记录代理密码。

## 下载和安装

### 应用更新（0.4.0）

在“设置 → 应用更新”中手动检查、查看新版说明、下载并安装更新。默认启动后检查一次，运行期间每 6 小时检查，可关闭自动检查。自动检查只提示，不会自动安装。

更新完成后首次启动弹出当前安装版本的更新内容；关闭后不再重复显示，也可以在设置中再次查看。更新说明随安装包分发，离线可看。首次安装同样显示当前版本说明。

0.3.0 及更早版本没有更新器，需手动安装 0.4.0 一次，后续即可应用内更新。更新安装会关闭 Codex Guard，安装器默认重启应用。便携版使用更新功能会启动安装程序，转为安装版。

从 [GitHub Releases](https://github.com/Hessel2333/codex-guard/releases/latest) 下载 Windows x64 版本：

- `*-setup.exe`：交互式安装程序，推荐普通用户使用。
- `*.msi`：Windows Installer 安装包。
- `*-portable.zip`：解压后运行 `codex-guard.exe`，需要已安装 Microsoft Edge WebView2 Runtime。
- `SHA256SUMS.txt`：发布文件的 SHA-256 校验值，可用 PowerShell `Get-FileHash -Algorithm SHA256 <文件路径>` 核对。

支持 Windows 10 / Windows 11。当前发布文件未进行代码签名。

## 运行和构建

直接运行：`src-tauri\target\release\codex-guard.exe`。

本地构建需要 Node.js、Rust MSVC 工具链和 Visual Studio C++ Build Tools；运行需要 WebView2 Runtime。

```powershell
npm ci
npm run tauri dev
```

仅 `npm run dev` 是浏览器预览，系统检测和代理操作需要桌面运行时。

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run test:ui
npm run tauri build -- --debug --no-bundle
npm run test:native
npm run tauri build -- --no-bundle
```

生成安装包：

```powershell
npm run tauri build
```

安装包输出到 `src-tauri/target/release/bundle/`，便携 EXE 位于 `src-tauri/target/release/codex-guard.exe`。

### 发布带签名的更新

更新 `src/release.json` 的版本和更新说明，并同步 package.json、package-lock.json、Cargo.toml、Cargo.lock、tauri.conf.json 版本。公钥固定在 Tauri 配置中，后续版本必须使用同一私钥签名。

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Join-Path $env:USERPROFILE '.tauri/codex-guard-updater.key'
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
npm run release
```

脚本生成 `release-artifacts/v<版本>/` 下的安装包、签名、`latest.json`、校验文件和发布说明。发布 GitHub Release 时必须同时上传安装包、`.sig` 和 `latest.json`，并设为最新正式版本。私钥保存在项目外，应安全备份；不能提交到 Git。更新器签名与 Windows Authenticode 代码签名不同。

验证最终 Release：

```powershell
$env:GUARD_EXE = 'src-tauri/target/release/codex-guard.exe'
npm run test:native
Remove-Item Env:GUARD_EXE
```

`test:native` 使用独立 WebView2 测试目录和临时 CDP 端口 19327。它读取本机真实状态、验证拒绝未确认命令并取消启动确认框，不结束真实 Codex。代理启动/重启/停止的原生生命周期由 Rust 测试在临时目录中的小程序验证；Windows .lnk 快捷方式也在临时目录验证，不改真实桌面。

项目 `.npmrc` 使用 PowerShell 运行 npm scripts，避免本机 npm/cmd shim 异常。所有系统逻辑位于 `src-tauri`，React 不拼接或执行 PowerShell。

## 代码结构

```text
src/components/             Overview / Diagnostics / ProxyLauncher / Settings / ConfirmDialog
src/lib/                    类型化 Tauri invoke 接口
src-tauri/src/windows.rs    AppX、用户环境与文件只读检测
src-tauri/src/model.rs      健康规则
src-tauri/src/proxy/        配置、启动、进程、UAC、快捷方式、日志
src-tauri/src/proxy_commands.rs
                           异步 Tauri command
src-tauri/test-fixtures/    无害子进程测试探针
```

更多内容：[架构](docs/architecture.md)、[代理功能验收](docs/proxy-verification.md)。

计划任务、更新后自动修复、完整 Repair History 尚未实现。当前支持手动一键修复 CODEX_CLI_PATH。
