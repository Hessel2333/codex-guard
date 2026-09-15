# CLI path repair implementation plan

**Goal:** Repair the current user's persistent CODEX_CLI_PATH from verified Codex package data.

**Architecture:** The overview requests confirmation of the displayed old and expected paths. An asynchronous native command detects again, rejects stale inputs and invalid installations, writes HKCU Environment, reads back, broadcasts an environment change, and returns fresh status. Inputs are preconditions, never arbitrary write targets. Existing processes must be restarted by the user.

**Tech stack:** Tauri 2, Rust, Win32/winreg, React, TypeScript, Playwright.

- [x] Implement native validation, serialized repair, registry write/readback, notification and structured result.
- [x] Add confirmation, progress, success/error feedback and refreshed status to the overview.
- [x] Test stale requests, invalid targets, denied confirmation, registry roundtrip in an isolated key, UI cancel/success/failure.
- [x] Run production build, Rust and UI tests, and final native smoke test; document behavior.

Automatic checks are a separate subsystem, pending scope clarification.
