import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

function base64(value) {
  const text = value.trim();
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(text) || !text) throw Error('Invalid base64');
  return Buffer.from(text, 'base64');
}

// Release-time verification only. The application still uses Tauri's native
// minisign verifier; this does not replace or relax runtime verification.
export function verifyUpdaterSignature(bytes, encodedSignature, encodedPublicKey) {
  const publicLines = base64(encodedPublicKey).toString('utf8').trim().split(/\r?\n/);
  const signatureLines = base64(encodedSignature).toString('utf8').trim().split(/\r?\n/);
  if (publicLines.length !== 2 || signatureLines.length !== 4 || !publicLines[0].startsWith('untrusted comment: ') || !signatureLines[0].startsWith('untrusted comment: ') || !signatureLines[2].startsWith('trusted comment: ')) throw Error('Invalid minisign envelope');
  const publicBytes = base64(publicLines[1]);
  const signatureBytes = base64(signatureLines[1]);
  const globalSignature = base64(signatureLines[3]);
  if (publicBytes.length !== 42 || signatureBytes.length !== 74 || globalSignature.length !== 64) throw Error('Invalid minisign length');
  if (!['Ed', 'ED'].includes(publicBytes.subarray(0, 2).toString()) || !['Ed', 'ED'].includes(signatureBytes.subarray(0, 2).toString())) throw Error('Unsupported minisign algorithm');
  if (!publicBytes.subarray(2, 10).equals(signatureBytes.subarray(2, 10))) throw Error('Updater signing key does not match configured public key');
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), publicBytes.subarray(10)]), format: 'der', type: 'spki' });
  const payload = signatureBytes.subarray(0, 2).toString() === 'ED' ? createHash('blake2b512').update(bytes).digest() : bytes;
  const signature = signatureBytes.subarray(10);
  if (!verify(null, payload, key, signature)) throw Error('Invalid updater artifact signature');
  if (!verify(null, Buffer.concat([signature, Buffer.from(signatureLines[2].slice(17))]), key, globalSignature)) throw Error('Invalid updater trusted-comment signature');
  return publicBytes.subarray(2, 10).toString('hex');
}

export async function verifyReleaseDirectory(directory, publicKey, version) {
  const feed = JSON.parse(await readFile(path.join(directory, 'latest.json'), 'utf8'));
  if (feed.version !== version) throw Error('Update feed version mismatch');
  const sums = new Map();
  for (const line of (await readFile(path.join(directory, 'SHA256SUMS.txt'), 'utf8')).trim().split(/\r?\n/)) {
    const match = /^([a-f0-9]{64})  ([^\\/]+)$/.exec(line);
    if (!match || sums.has(match[2])) throw Error('Invalid or duplicate checksum entry');
    sums.set(match[2], match[1]);
  }
  const verified = [];
  for (const [kind, suffix] of [['nsis', 'x64-setup.exe'], ['msi', 'x64_en-US.msi']]) {
    const name = `Codex-Guard_${version}_${suffix}`;
    const entry = feed.platforms?.[`windows-x86_64-${kind}`];
    if (entry?.url !== `https://github.com/Hessel2333/codex-guard/releases/download/v${version}/${name}`) throw Error('Unexpected update artifact URL');
    const signature = (await readFile(path.join(directory, name + '.sig'), 'utf8')).trim();
    if (entry.signature !== signature) throw Error('Update feed signature differs from sidecar');
    const keyId = verifyUpdaterSignature(await readFile(path.join(directory, name)), signature, publicKey);
    for (const file of [name, name + '.sig', 'latest.json']) if (!sums.has(file)) throw Error('Missing checksum: ' + file);
    verified.push({ name, keyId });
  }
  if (JSON.stringify(feed.platforms['windows-x86_64']) !== JSON.stringify(feed.platforms['windows-x86_64-nsis'])) throw Error('Default Windows platform must use NSIS');
  for (const [name, hash] of sums) if (createHash('sha256').update(await readFile(path.join(directory, name))).digest('hex') !== hash) throw Error('Checksum mismatch: ' + name);
  return { version, signatures: verified, checksums: sums.size };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const config = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'));
  console.log(JSON.stringify(await verifyReleaseDirectory(process.argv[2] || `release-artifacts/v${config.version}`, config.plugins.updater.pubkey, config.version), null, 2));
}
