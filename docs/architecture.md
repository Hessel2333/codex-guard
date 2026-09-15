# Codex Guard

## 当前交付边界

0.2.0 保留 Phase 1 + Phase 2 的启动健康检测，并集成 D:\Codes\codex\_proxy 的普通/管理员代理启动、停止/重启、进程状态、日志、配置与桌面快捷方式。持久化环境修复、计划任务和更新历史仍在后续阶段。

## 分层

```text
React + TypeScript
  ├─ Overview / Diagnostics / Theme settings
  └─ typed invoke → get_codex_status()
                         ↓ blocking worker
Rust service → pure health policy
  ├─ Windows Runtime PackageManager (当前用户，main packages)
  ├─ HKCU\\Environment，KEY_READ，仅 CODEX_CLI_PATH
  └─ std::fs + Windows version APIs（只读文件）
```

所有系统操作属于 `src-tauri`。前端不执行 shell、不解析 PowerShell 文本。Rust 返回 `Serialize` 数据及统一 `AppError { code, message, detail }`。无包是正常检测状态；访问拒绝、AppX 服务错误、注册表读取错误不能被误报为未安装或健康。

版本比较使用四段数字元组；不用字符串排序。用户变量从持久化 HKCU 读取，不能使用当前进程继承的过期变量。文件不存在和无法访问分别表示。路径比较使用 Windows 大小写不敏感比较，统一斜杠，不擅自去掉引号或展开非法值。

## 后续阶段

3. 完善健康首页和操作区。
4. 修复服务：重新检测 → 检查 bundled CLI → 写当前用户变量 → SendMessageTimeoutW 广播 → 重新检测；JSON 历史保存 old/new/result/error。
5. Safe Launch：复用修复服务，从当前包 manifest 解析 Application ID，使用系统激活 API；不硬编码 AUMID。
6. 同一 EXE `--repair --silent` 在创建 WebView 前进入无界面服务；计划任务每五分钟及当前用户登录执行。比复制 PowerShell 脚本更易维护，所有入口共享校验和日志。schtasks 使用独立参数/XML，不拼接用户输入；只管理本应用有所有权记录的任务，创建后实际运行并验证 Last Run Result。首次权限失败才提示，不自动提权。
7. `%LOCALAPPDATA%\\CodexBootGuard` JSON 原子写入及跨进程锁；更新历史、缓存、进程、Windows/计划任务完整诊断。
8. 高级操作确认、可访问性、错误恢复、安装与升级检查。

## 安全边界

普通用户运行；不改 WindowsApps ACL/内容，不接管所有权，不读写其他用户配置。健康检测只读 HKCU；代理配置写入本应用的 LocalAppData。代理启动/停止先确认，Rust 再验证当前会话、路径及进程创建时间。管理员启动通过同一 EXE 的无 WebView helper 请求 UAC，主窗口不自动提权。

## 0.2.0 代理启动分层

ProxyLauncher / ConfirmDialog → proxy_commands.rs 阻塞工作线程 → proxy/settings.rs（配置校验）、proxy/mod.rs（发现、启动、状态、日志、文件锁）、proxy/process.rs（原生进程与句柄验证）、proxy/shell.rs（Known Folders、UAC helper、原生快捷方式）。

普通启动使用 Command.env 构造独立子进程环境。管理员模式使用 ShellExecuteExW runas 启动当前 EXE 的 --proxy-elevated 入口，helper 复用相同 Rust 服务。配置先保存，避免 UAC 不继承代理环境；Windows Known Folder 校验同一用户目录。按请求隔离的 JSON 返回 helper 结果，父进程等待实际退出，不把 UAC 请求成功当成 Codex 启动成功。

新进程确认须匹配选定 EXE、创建时间和本次 launcher 的父子关系。两秒存活检查验证启动，不代表代理网络连通性测试。

## 实现参考

- https://v2.tauri.app/start/prerequisites/
- https://microsoft.github.io/windows-docs-rs/doc/windows/Management/Deployment/struct.PackageManager.html

## 视觉约定

参考 `docs/design-concept.png`：Segoe UI、浅灰背景 #f5f6f8、白色卡片、10px 圆角、低对比边框、克制的青绿色强调。顶栏品牌/Refresh/Settings；Overview 和 Diagnostics 横向标签；状态主卡、三个事实列、路径对比区和底部检测时间。所有状态来自真实 Rust 结果。参考图中的路径仅为设计样例，运行时必须替换为完整真实 bundled CLI 路径；深色、加载、缺失、错误状态沿用同一组件体系。
