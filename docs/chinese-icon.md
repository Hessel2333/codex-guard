# 中文界面与角标图标

默认界面为简体中文（HTML `zh-CN`），覆盖导航、概览、诊断字段、代理表单、状态、设置、确认提示与常见错误标题。程序名称、环境变量、路径、错误码和原始日志保持原样，以便排查问题。日期使用中文格式。

图标以用户提供的蓝紫色 Codex 参考图为依据，使用内置 imagegen 重绘主体，并在右下角添加盾牌勾选小标；并非从原始矢量文件直接叠加。最终采用纯白底，避免生成结果中的棋盘格背景进入成品。项目源图为 `public/guard.png`，Tauri icon 工具生成各尺寸 PNG、ICO；应用、侧栏和两种快捷方式使用一致图标。普通与管理员快捷方式通过名称区别。

最终提示词（内置 imagegen 编辑模式）：

> Keep this icon EXACTLY as is, cloud, terminal glyph, small shield badge. Remove ALL gray checkerboard from the image. Background including all four corners must be SOLID PURE WHITE #FFFFFF, no transparency checkerboard, no texture, no noise. Clean square app icon with white background, edge to edge. No other changes.

初始提示指定保留参考图蓝紫色云状主体和白色终端符号，在右下角添加深蓝圆形盾牌勾选角标，直径约为图标宽度的 22%，无额外文字。

验证：生产前端构建及 14 项界面测试通过；确认弹窗取消不会调用启动/停止操作，中文错误标题仍保留原始错误码和技术详情。

发布 EXE 构建成功，实际 Tauri/WebView2 冒烟测试通过；已检查浅色概览及深色代理页截图，应用包和环境变量结果与只读系统查询一致。未重启用户的 Codex 会话。
