import { readFile, writeFile, copyFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';

// Keep the generated artwork square. Apply transparent rounding only here.
const source = await readFile('src-tauri/icons/source-square.png');
if (source.readUInt32BE(16) !== source.readUInt32BE(20)) throw Error('Icon source must be square');
const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1024" height="1024" viewBox="0 0 1024 1024"><defs><clipPath id="tile"><rect width="1024" height="1024" rx="224"/></clipPath></defs><g clip-path="url(#tile)"><rect width="1024" height="1024" fill="#fff"/><image width="1024" height="1024" preserveAspectRatio="xMidYMid meet" xlink:href="data:image/png;base64,${source.toString('base64')}"/></g></svg>`;
await writeFile('src-tauri/icons/app-icon.svg', svg);
const result = spawnSync(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', 'icon', 'src-tauri/icons/app-icon.svg'], { stdio: 'inherit' });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
await copyFile('src-tauri/icons/icon.png', 'public/guard.png');
await copyFile('src-tauri/icons/icon.ico', 'src-tauri/icons/proxy.ico');
await copyFile('src-tauri/icons/icon.ico', 'src-tauri/icons/proxy-admin.ico');
