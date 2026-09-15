# 代理功能集成验收（0.2.0）

源目录：D:\Codes\codex\_proxy。已读取 README、codex-proxy.ps1、两个 cmd 启动器和 install-shortcuts.ps1，并复用 P/A 图标。新应用不调用原脚本；原目录未修改。

## 功能对应

普通/管理员代理启动、重启、停止、进程状态、新旧日志切换、可保存的代理配置、CODEX_APP 路径覆盖、桌面快捷方式均已实现。默认端口 7890，可编辑为其他端口。快捷方式进入启动确认页；旧脚本快捷方式不被覆盖。

代理变量仅注入新进程；bundled CLI 存在时还传递正确 CODEX_CLI_PATH。不写系统代理、用户代理变量或 WindowsApps。管理员启动单独请求 UAC，主 Tauri 窗口保留普通权限。

## 已实际验证

- cargo check、前端 TypeScript / Vite build 通过。
- 23 项 Rust 测试通过。
- 14 项 Playwright / Edge 界面测试通过。
- Tauri Debug 构建和实际 WebView2 native smoke test 通过。
- 最终 Release 构建与 Release EXE 原生 smoke test 通过；文件版本 0.2.0，大小 4,391,936 bytes。
- 实际页面截图：[浅色](proxy.png)、[深色](proxy-dark.png)。

Rust 测试在临时目录编译无害 Codex.exe 探针，完整执行生产启动、重启、停止服务。探针实际读取 HTTP_PROXY、HTTPS_PROXY、ALL_PROXY、NO_PROXY 和对应 CODEX_CLI_PATH，逐项断言；父进程环境保持原值。配置、日志、启动记录全部写入测试临时目录，目录替换仅测试编译启用，正式应用没有测试目录后门。

进程操作只针对临时探针，不结束本机真实 Codex。原生 COM 快捷方式在临时目录创建、更新参数并读回验证；目标冲突返回错误，不修改真实桌面。未确认的启动、停止、管理员启动在系统操作前返回 CONFIRMATION_REQUIRED。另覆盖 URL/端口校验、Windows 参数转义、路径边界、同级无关 EXE 和独立启动会话。

UI 测试覆盖配置保存、未保存状态禁止启动、取消停止/重启无副作用、确认后发送结构化命令、UAC 取消展示错误、旧日志、快捷方式和 680px 布局。快捷方式启动意图只打开确认框，不自动执行。

原生 WebView2 smoke test 读取本机真实路径和进程，验证 proxy command 可调用，验证未确认命令被拒绝，并打开/取消管理员启动确认框。

## 核验边界

没有在当前工作会话中重启真实 Codex，也未弹出真实 UAC 提权启动 Codex，以免中断任务。普通进程生命周期与环境注入通过原生探针验证；真实 UAC、打包 Codex 以管理员身份启动及实际代理网络连通性，需在用户主动启动时验证。产品不会把已有会话标记为代理已生效，也不将 UAC 请求成功当作 Codex 启动成功。

## 视觉验证

沿用现有 Segoe UI、白/深灰卡片、青绿强调、圆角、状态点和原生标题栏。会话没有 Browser/IAB 工具，因此使用 Playwright / 系统 Edge，并通过 CDP 采集实际 Tauri WebView2。已用 view_image 查看浅色和深色页面。

已检查布局、字体层级、配色、确认框焦点/取消、长路径、日志滚动、680px 宽度和两种主题。进程列表限制高度并可滚动，避免 Electron 多进程遮蔽日志和快捷方式。路径、PID、版本来自真实检测；新页面沿用 design-concept.png 的设计系统。
