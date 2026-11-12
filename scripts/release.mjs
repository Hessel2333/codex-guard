import { readFile, writeFile, mkdir, copyFile, mkdtemp, unlink, rmdir, readdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
import { verifyUpdaterSignature, verifyReleaseDirectory } from './verify-updater.mjs';

const release = JSON.parse(await readFile('src/release.json', 'utf8'));
const config = JSON.parse((await readFile('src-tauri/tauri.conf.json', 'utf8')).replace(/^\uFEFF/, ''));
const pkg = JSON.parse(await readFile('package.json', 'utf8'));
const cargo = await readFile('src-tauri/Cargo.toml', 'utf8');
const cargoVersion = cargo.match(/\[package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/)?.[1];
if (release.version !== config.version || release.version !== pkg.version || release.version !== cargoVersion || !release.notes.trim()) throw Error('Release versions and notes must match');
if (!process.env.TAURI_SIGNING_PRIVATE_KEY) throw Error('Set TAURI_SIGNING_PRIVATE_KEY, or use npm run release:windows to unlock the encrypted local key');
// Windows PowerShell removes environment entries assigned ''. Ensure Tauri sees
// an explicit empty password rather than prompting on a non-interactive pipe.
process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= '';
// Normalize paths to content for the standalone signer. Neither is logged.
try { process.env.TAURI_SIGNING_PRIVATE_KEY = (await readFile(process.env.TAURI_SIGNING_PRIVATE_KEY, 'utf8')).trim(); } catch (error) { if (!['ENOENT', 'EINVAL', 'ENAMETOOLONG'].includes(error.code)) throw Error('Unable to read signing key input; details withheld to protect secrets.'); }
const temporary = await mkdtemp(path.join(os.tmpdir(), 'codex-guard-signing-check-'));
const probe = path.join(temporary, 'probe.txt');
try {
  await writeFile(probe, 'Codex Guard release signing preflight\n');
  const result = spawnSync(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', 'signer', 'sign', probe], { encoding: 'utf8', windowsHide: true, timeout: 30000 });
  if (result.status !== 0) throw Error('Signing preflight failed. Check the key and password; signer output is withheld to protect secrets.');
  verifyUpdaterSignature(await readFile(probe), (await readFile(probe + '.sig', 'utf8')).trim(), config.plugins.updater.pubkey);
} finally {
  for (const file of [probe, probe + '.sig']) await unlink(file).catch(error => { if (error.code !== 'ENOENT') throw error; });
  await rmdir(temporary);
}
console.log('Signing preflight passed: key matches the application public key.');
const dir = path.resolve(`release-artifacts/v${release.version}`);
await mkdir(dir, { recursive: true });
if ((await readdir(dir)).some(name => /\.(exe|msi|zip|sig)$/.test(name) || name === 'latest.json')) throw Error('Release output already contains artifacts; do not overwrite published builds. Use a new version or a new output directory.');
const override = path.join(dir, 'build-config.json');
await writeFile(override, JSON.stringify({ build: { beforeBuildCommand: '' } }));
for (const args of [['scripts/build.mjs'], ['node_modules/@tauri-apps/cli/tauri.js', 'build', '--ci', '--config', override]]) {
  const result = spawnSync(process.execPath, args, { stdio: 'inherit', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const platforms = {};
const files = [];
for (const [kind, suffix] of [['nsis', 'x64-setup.exe'], ['msi', 'x64_en-US.msi']]) {
  const source = `src-tauri/target/release/bundle/${kind}/Codex Guard_${release.version}_${suffix}`;
  const name = `Codex-Guard_${release.version}_${suffix}`;
  await copyFile(source, path.join(dir, name));
  await copyFile(`${source}.sig`, path.join(dir, `${name}.sig`));
  const signature = (await readFile(`${source}.sig`, 'utf8')).trim();
  platforms[`windows-x86_64-${kind}`] = { signature, url: `https://github.com/Hessel2333/codex-guard/releases/download/v${release.version}/${name}` };
  files.push(name, `${name}.sig`);
}
platforms['windows-x86_64'] = platforms['windows-x86_64-nsis'];
await writeFile(path.join(dir, 'latest.json'), JSON.stringify({ version: release.version, notes: release.notes, pub_date: new Date().toISOString(), platforms }, null, 2));
files.push('latest.json');
await copyFile('src-tauri/target/release/codex-guard.exe', path.join(dir, 'codex-guard.exe'));
files.push('codex-guard.exe');
const portable = path.join(dir, 'portable');
await mkdir(portable);
await copyFile(path.join(dir, 'codex-guard.exe'), path.join(portable, 'codex-guard.exe'));
await writeFile(path.join(portable, 'README.txt'), `Codex Guard ${release.version}\r\n运行 codex-guard.exe，需要 WebView2 Runtime。\r\n使用应用内更新将安装为当前用户的桌面应用。\r\n`);
const archive = `Codex-Guard_${release.version}_x64-portable.zip`;
const quote = value => `'${value.replaceAll("'", "''")}'`;
const zipCommand = `$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::CreateFromDirectory(${quote(portable)}, ${quote(path.join(dir, archive))}, [System.IO.Compression.CompressionLevel]::Optimal, $false)`;
const zipResult = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(zipCommand, 'utf16le').toString('base64')], { stdio: 'inherit', windowsHide: true });
if (zipResult.status !== 0) throw Error('Portable ZIP creation failed');
files.push(archive);
const sums = await Promise.all(files.map(async name => `${createHash('sha256').update(await readFile(path.join(dir, name))).digest('hex')}  ${name}`));
await writeFile(path.join(dir, 'SHA256SUMS.txt'), `${sums.join('\n')}\n`);
await writeFile(path.join(dir, 'release-notes.md'), `# Codex Guard ${release.version}\n\n${release.notes}\n\nWindows 10 / 11 x64。\n\n## 旧版升级\n\n0.5.0 及更早版本使用的旧更新私钥已丢失，不能通过旧签名链自动升级。请手动安装 0.5.1 或更新版本一次，保留已有配置；之后可正常应用内更新。不会关闭签名校验。\n\n安装包包含 Tauri 更新签名，未进行 Windows Authenticode 代码签名。\n`);
console.log(JSON.stringify(await verifyReleaseDirectory(dir, config.plugins.updater.pubkey, release.version), null, 2));
console.log(`Release assets prepared in ${dir}`);
