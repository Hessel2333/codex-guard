# Launcher workspace update · 2026-09-14

The desktop layout follows `D:\Codes\PP_ML\desktop\web\index.html`, `style.css` and `assets\tokens.css`: a persistent left sidebar, gray workspace, blue selected navigation and controls, 14px rounded panels, subtle shadows and a separate page heading. Existing health checks, proxy controls, settings and light/dark themes remain available. Small windows collapse the layout without horizontal page scrolling.

Runtime inspection found no PowerShell or cmd invocation: package discovery, registry reads, process checks, shortcuts and elevation use native Rust/Windows APIs. Both debug and release entry points now use the Windows GUI subsystem. Proxy child creation uses `CREATE_NO_WINDOW`; the isolated console-subsystem probe asserts that `GetConsoleWindow` returns null before testing start/restart/stop. This flag does not hide desktop GUI applications, as specified in [Microsoft's process creation documentation](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags).

Checks and log refresh run asynchronously in the background. System UAC remains the standard Windows authorization flow. Runtime does not execute the original PowerShell launcher scripts. Existing process termination confirmations remain in place.

Validation: frontend production build, 14 Playwright UI tests (including narrow widths and confirmation cancellation), and 23 Rust tests passed. The Rust lifecycle test uses only an isolated temporary probe, never the active Codex desktop session.

The release EXE built successfully and passed the actual Tauri/WebView2 smoke test, including PE GUI-subsystem validation, native status checks, navigation, themes and cancellation. Screenshots were visually inspected and refreshed in `overview.png`, `diagnostics.png`, `dark.png`, `proxy.png` and `proxy-dark.png`. Actual Codex restart, UAC approval and external proxy traffic were not exercised.
