# Phase 1 + Phase 2 验收

## 已实际执行

- Phase 1：脚手架 `cargo check`、前端 build、`tauri build --debug --no-bundle` 通过；窗口标题 Codex Guard，MainWindowHandle 非零，Responding=True。
- Phase 2：`cargo check` 通过；`npm run build` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`：11/11 通过。
- `npm run test:ui`：7/7 通过，使用本机 Microsoft Edge。
- `cargo run --manifest-path src-tauri/Cargo.toml --example detect`：真实原生检测成功。
- `npm run test:native`：在实际 Tauri WebView2 中调用 Rust command；与独立 PowerShell current-user AppX / User 环境变量结果一致；刷新、Diagnostics、浅深主题、Escape 关闭设置通过，无 JavaScript 异常。
- `npm run tauri build -- --no-bundle`：Release 构建成功，EXE 4,154,880 bytes；针对 Release EXE 再次执行原生 smoke test 通过，无需 Vite 开发服务器。
- 最终实际截图：[Overview](overview.png)、[Dark](dark.png)、[Diagnostics](diagnostics.png)。

## 本机结果

- OpenAI.Codex：26.908.4834.0。
- 当前用户 CODEX_CLI_PATH 与该包的 `app\resources\codex.exe` 一致。
- ExpectedExists=true、CurrentExists=true、PathMatches=true、Readable=true。
- CLI 大小：297,858,352 bytes。
- Health=healthy；issues=[]。
- 此 CLI 没有可获取的文件版本资源，界面显示不可用；包版本仍可正常读取。

检测全过程不写真实用户环境变量，不注册计划任务，不结束 ChatGPT/Codex 进程。测试只启动并结束本项目自己的 EXE。

## 场景覆盖与边界

| 场景 | 本阶段覆盖 |
| --- | --- |
| A 路径正确 | Rust 单元测试、UI 测试、本机真实集成验证 |
| B 旧路径不存在 | Rust 单元测试 + UI 警告；修复动作属于 Phase 4 |
| C 更新后自动修复 | 尚未实现，属于 Phase 6/7 |
| D 未安装 | Rust 单元测试 + UI 空状态 |
| E bundled CLI 缺失 | Rust 单元测试 + UI 安装不完整错误 |
| F 任务创建失败 | 尚未实现，属于 Phase 6 |
| 访问拒绝 | Rust unknown 状态及 UI HRESULT 显示/重试测试 |
| 用户变量为空、指向目录、指向存在的错误版本 | Rust 单元测试 |
| 浏览器直接打开 | 显示需要 Tauri desktop，不伪造健康数据 |

## 视觉核验

设计参考：`design-concept.png`，由内置 Image Gen 生成。实现截图通过 Playwright CDP 直接采集 Tauri 的 WebView2；因为会话无 Browser/IAB 工具，采用 Playwright + 系统 Edge。已使用 view_image 查看参考、浅色成品和深色成品。

| 比较点 | 参考与实际结果 |
| --- | --- |
| 信息结构 | 品牌/两个按钮、横向 Overview/Diagnostics、主状态卡、路径卡和底部时间，均保留 |
| 文案 | 主状态和标签沿用参考；版本、完整路径和检测时间替换为真实数据 |
| 字体 | Segoe UI；状态标题最突出，字段标签次级，长路径使用等宽字体 |
| 色彩 | 浅灰底、白卡、低对比边框、青绿强调；成功/警告/错误有不同色彩和文本 |
| 卡片与间距 | 圆角主卡、三列事实、路径两行；单一系统工具视图，没有后台侧栏或无关统计图 |
| 图标 | shield、refresh、settings、chevron、check 均为可缩放线性 SVG |
| 交互与尺寸 | 实际桌面窗口 + 1490/1100/680/390 CSS px 宽度测试，无横向溢出；长路径允许换行 |

首屏 copy diff：仅修正参考中示例路径（参考图未包含真实 publisher 和 app/resources 层级），使用后端实际值；检测时间增加秒方便验证刷新。这是需求要求的动态数据差异。错误/未安装/深色状态沿用相同组件，未将参考的绿色健康状态硬编码。窗口按钮使用系统原生标题栏，因此截图只包含 WebView 内容。Windows 125% DPI 下原生截图尺寸为 1356×1040，参考图片为 1490×1056；同时完成不同 CSS viewport 的布局检查。

已对照设计完成结构、文案、字体、色彩、图标、边框和长路径检查，未发现遮挡、裁切、无效控件或需修复的主要视觉偏差。
