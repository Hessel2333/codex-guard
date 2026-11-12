import { readFile, writeFile, mkdir, copyFile, mkdtemp, unlink, rmdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import os from 'node:os';

// Local installers are deliberately separate from signed updater releases.
// The checked-in updater public key and release configuration remain intact.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
process.chdir(root);
const { version } = JSON.parse(await readFile('package.json', 'utf8'));
const stamp = new Intl.DateTimeFormat('sv-SE', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false }).format(new Date()).replace(/\D/g, '');
const out = path.join(root, 'release-artifacts', `local-${version}-${stamp}`);
const temporary = await mkdtemp(path.join(os.tmpdir(), 'codex-guard-local-package-'));
const override = path.join(temporary, 'local-build.json');
await writeFile(override, JSON.stringify({ build: { beforeBuildCommand: '' }, bundle: { createUpdaterArtifacts: false } }));
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${path.basename(command)} failed (${result.status})`);
}
try {
  run(process.execPath, ['scripts/build.mjs']);
  run(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', 'build', '--config', override]);
  await mkdir(out, { recursive: true });
  const files = [];
  for (const [source, name] of [
    ['src-tauri/target/release/codex-guard.exe', 'codex-guard.exe'],
    [`src-tauri/target/release/bundle/nsis/Codex Guard_${version}_x64-setup.exe`, `Codex-Guard_${version}_x64-setup.exe`],
    [`src-tauri/target/release/bundle/msi/Codex Guard_${version}_x64_en-US.msi`, `Codex-Guard_${version}_x64.msi`],
  ]) { await copyFile(source, path.join(out, name)); files.push(name); }
  const portable = path.join(out, 'portable');
  await mkdir(portable);
  await copyFile(path.join(out, 'codex-guard.exe'), path.join(portable, 'codex-guard.exe'));
  await writeFile(path.join(portable, 'README.txt'), 'Codex Guard 本地构建\r\n运行 codex-guard.exe，需要 Microsoft Edge WebView2 Runtime。\r\n桌面配置中的“启动此配置”可同时使用代理启动器中已保存的代理。\r\n本地安装包未作 Authenticode 签名，不用于自动更新发布。\r\n');
  const archive = `Codex-Guard_${version}_x64-portable.zip`;
  const quote = value => `'${value.replaceAll("'", "''")}'`;
  // Use .NET directly: Windows PowerShell's Archive module may be blocked by
  // the local module policy even though the runtime supports ZIP creation.
  const powershell = `$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory(${quote(portable)}, ${quote(path.join(out, archive))}, [System.IO.Compression.CompressionLevel]::Optimal, $false)`;
  run('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(powershell, 'utf16le').toString('base64')]);
  files.push(archive);
  const checksums = {};
  for (const file of files) checksums[file] = createHash('sha256').update(await readFile(path.join(out, file))).digest('hex');
  await writeFile(path.join(out, 'SHA256SUMS.txt'), Object.entries(checksums).map(([file, sha]) => `${sha}  ${file}`).join('\n') + '\n');
  await writeFile(path.join(out, 'build.json'), JSON.stringify({ version, built_at: new Date().toISOString(), kind: 'local', updater_artifacts: false, files: checksums }, null, 2));
  console.log(`LOCAL_PACKAGE_DIR=${out}`);
} finally {
  await unlink(override);
  await rmdir(temporary); // Non-recursive: never remove an unexpected directory tree.
}
