import { readFile, writeFile, mkdir, copyFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import path from 'node:path';

const release = JSON.parse(await readFile('src/release.json', 'utf8'));
const config = JSON.parse((await readFile('src-tauri/tauri.conf.json', 'utf8')).replace(/^\uFEFF/, ''));
const pkg = JSON.parse(await readFile('package.json', 'utf8'));
const cargo = await readFile('src-tauri/Cargo.toml', 'utf8');
const cargoVersion = cargo.match(/\[package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/)?.[1];
if (release.version !== config.version || release.version !== pkg.version || release.version !== cargoVersion || !release.notes.trim()) throw Error('Release versions and notes must match');
if (!process.env.TAURI_SIGNING_PRIVATE_KEY) throw Error('Set TAURI_SIGNING_PRIVATE_KEY to your private signing key path');
const dir = path.resolve(`release-artifacts/v${release.version}`);
await mkdir(dir, { recursive: true });
const override = path.join(dir, 'build-config.json');
await writeFile(override, JSON.stringify({ build: { beforeBuildCommand: '' } }));
for (const args of [['scripts/build.mjs'], ['node_modules/@tauri-apps/cli/tauri.js', 'build', '--config', override]]) {
  const result = spawnSync(process.execPath, args, { stdio: 'inherit' });
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
const sums = await Promise.all(files.map(async name => `${createHash('sha256').update(await readFile(path.join(dir, name))).digest('hex')}  ${name}`));
await writeFile(path.join(dir, 'SHA256SUMS.txt'), `${sums.join('\n')}\n`);
await writeFile(path.join(dir, 'release-notes.md'), `# Codex Guard ${release.version}\n\n${release.notes}\n\nWindows 10 / 11 x64。首次从 0.3.0 或更早版本升级需手动安装本版，后续可在应用内更新。安装包包含 Tauri 更新签名，未进行 Windows Authenticode 代码签名。\n`);
console.log(`Release assets prepared in ${dir}`);
