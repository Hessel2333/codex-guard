# 中文界面与角标图标

## 正方形源图与透明裁剪（0.4.3）

当前使用纯白底正方形源图 `src-tauri/icons/source-square.png`，生成时不包含圆角、外框或外部阴影。运行 `node scripts/icons.mjs`，将源图等比放入 1024×1024 画布，再以半径 224 的圆角裁剪，生成透明四角和各平台图标。应用内 PNG 与 Windows ICO 使用同一输出。

修复侧栏图标 CSS 原来的 36×42、紧凑窗口 27×42 尺寸，分别改为 36×36、27×27，并设置等比显示。界面测试覆盖四种窗口宽度，断言图标宽高相等。

## 原版主体比例修正（0.4.2）

0.4.1 虽然等比缩放，但沿用的重绘主体已与用户原版参考图轮廓不同。0.4.2 直接使用用户提供的 1024×576 参考图中的图标区域（x=420、y=128，184×184），SVG 中原图保持 1024×576 的原始宽高，平移裁取后整体等比输出，不再重绘。盾牌角标独立等比叠加。参考图本身图标区域分辨率较低，大尺寸预览会较柔和。

## 圆角图标（0.4.1）

按原版 Codex 参考图的占比减少白色留边，原始图案整体相对放大 512/448（约 14.3%），不改变图案内部比例。正式源文件为 `src-tauri/icons/app-icon.svg`：嵌入原始 PNG，使用圆角矩形裁剪，Tauri icon 命令生成多尺寸图标。四角 alpha=0，白底中心边缘 alpha=255。应用内图标及 proxy/proxy-admin ICO 同步。

内置 imagegen 用于圆角外形预览；最终采用原始图案和 SVG 几何轮廓，避免重绘改变细节和生成图中的假透明背景。预览提示：保留蓝紫色终端图案与盾牌角标，参照原版图标比例缩小留白，白底圆角矩形，四角透明。

默认界面为简体中文（HTML `zh-CN`），覆盖导航、概览、诊断字段、代理表单、状态、设置、确认提示与常见错误标题。程序名称、环境变量、路径、错误码和原始日志保持原样，以便排查问题。日期使用中文格式。

图标以用户提供的蓝紫色 Codex 参考图为依据，使用内置 imagegen 重绘主体，并在右下角添加盾牌勾选小标；并非从原始矢量文件直接叠加。最终采用纯白底，避免生成结果中的棋盘格背景进入成品。项目源图为 `public/guard.png`，Tauri icon 工具生成各尺寸 PNG、ICO；应用、侧栏和两种快捷方式使用一致图标。普通与管理员快捷方式通过名称区别。

最终提示词（内置 imagegen 编辑模式）：

> Keep this icon EXACTLY as is, cloud, terminal glyph, small shield badge. Remove ALL gray checkerboard from the image. Background including all four corners must be SOLID PURE WHITE #FFFFFF, no transparency checkerboard, no texture, no noise. Clean square app icon with white background, edge to edge. No other changes.

初始提示指定保留参考图蓝紫色云状主体和白色终端符号，在右下角添加深蓝圆形盾牌勾选角标，直径约为图标宽度的 22%，无额外文字。

验证：生产前端构建及 14 项界面测试通过；确认弹窗取消不会调用启动/停止操作，中文错误标题仍保留原始错误码和技术详情。

发布 EXE 构建成功，实际 Tauri/WebView2 冒烟测试通过；已检查浅色概览及深色代理页截图，应用包和环境变量结果与只读系统查询一致。未重启用户的 Codex 会话。
