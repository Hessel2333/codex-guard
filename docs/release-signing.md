# 更新签名与发布

## 签名链迁移

0.5.0 及更早版本的原更新私钥已丢失。公钥无法还原私钥，也不能签出旧客户端信任的更新。自 **0.5.1** 起使用新的更新签名密钥；旧客户端必须手动安装 0.5.1 或更新版本一次，安装保留已有设置。随后走正常的应用内更新。

没有关闭签名验证、信任任意公钥或允许不安全更新源。Tauri 更新签名不等于 Windows Authenticode 签名；目前没有 Authenticode 证书。

## 本机密钥

- 加密私钥：`%USERPROFILE%\.tauri\codex-guard\updater-v2.key.dpapi`。
- 公钥：同目录 `updater-v2.key.pub`；应用信任锚是 `src-tauri/tauri.conf.json` 中的 `plugins.updater.pubkey`。
- 加密备份：`%LOCALAPPDATA%\CodexGuardSigningBackup\`。
- 使用 Windows DPAPI CurrentUser 加密，目录仅当前 Windows 用户和 SYSTEM 可访问。构建时仅在子进程环境中解密，不输出私钥、不写入仓库或安装包。
- 备份已经过解密回读验证，但它是**同机、同账户备份**，不能代替异机灾备。Windows 账户/DPAPI 凭据丢失将导致这些副本不可解密。迁移机器前需要在旧账户下将私钥导出到用户自行管理的安全离线存储。

只在显式首次建钥时运行以下命令。正常发布缺少密钥时会失败，不会自动建钥或悄悄轮换。

```powershell
powershell.exe -NoProfile -NonInteractive -File scripts/release-windows.ps1 -InitializeSigningKey
```

初始化不会修改应用公钥。任何未来的轮换都必须明确安排客户端信任链迁移，不能只替换配置。

## 正式发布

同步 `src/release.json`、npm/Cargo 版本、Tauri 配置和 README 后：

```powershell
npm run test:release
npm run release:windows
npm run release:verify
```

也支持显式设置 `TAURI_SIGNING_PRIVATE_KEY`（内容或路径）和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 后运行 `npm run release`。

发布脚本在编译前签名一个临时探针，并用应用内公钥验证；错钥在耗时构建前即失败。构建后再次校验 NSIS、MSI 的完整签名、可信注释、更新 JSON、下载 URL、旁路签名文件与全部 SHA-256。拒绝覆盖已有发布构建。

将生成目录中的安装包、便携 ZIP、`.sig`、`latest.json` 和 `SHA256SUMS.txt` 上传至对应 GitHub Release，使用同一源码提交创建标签，确认所有文件完整后才设为最新正式版。不要上传密钥、DPAPI 文件、WebView 用户目录或日志。

## 验收边界

`npm run test:release` 覆盖有效签名、错钥、相同 key ID 的错误公钥、安装包篡改、可信注释篡改、无效编码、版本与校验清单漂移。应用运行时仍由 Tauri 的原生验签器负责，发布前还需使用已安装旧版从公开 GitHub 更新源完成一次真正的检查、下载、验签、安装、重启，随后再检查已是最新版本。

自动检查只提示，不会未经确认安装；安装只更新 Codex Guard，不重启 Codex 或修改其指令与代理配置。
