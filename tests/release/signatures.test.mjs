import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, randomBytes, sign } from 'node:crypto';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { verifyUpdaterSignature, verifyReleaseDirectory } from '../../scripts/verify-updater.mjs';

function fixture(prehashed = true, keyId = randomBytes(8)) {
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const content = Buffer.from('installer fixture, never executed');
  const pub = Buffer.concat([Buffer.from('Ed'), keyId, publicKey.export({ format: 'der', type: 'spki' }).subarray(-32)]);
  const signature = sign(null, prehashed ? createHash('blake2b512').update(content).digest() : content, privateKey);
  const comment = 'timestamp:1\tfile:fixture\tprehashed';
  const envelope = ['untrusted comment: test signature', Buffer.concat([Buffer.from(prehashed ? 'ED' : 'Ed'), keyId, signature]).toString('base64'), 'trusted comment: ' + comment, sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey).toString('base64')].join('\n');
  return { content, key: Buffer.from('untrusted comment: test public key\n' + pub.toString('base64')).toString('base64'), signature: Buffer.from(envelope).toString('base64'), keyId };
}

for (const prehashed of [true, false]) test(`accepts valid ${prehashed ? 'prehashed' : 'legacy'} minisign`, () => {
  const f = fixture(prehashed);
  assert.equal(verifyUpdaterSignature(f.content, f.signature, f.key), f.keyId.toString('hex'));
});

test('rejects an unrelated signing key', () => {
  const f = fixture();
  assert.throws(() => verifyUpdaterSignature(f.content, f.signature, fixture().key), /key does not match/);
});

test('rejects wrong public key even with the same key ID', () => {
  const f = fixture();
  assert.throws(() => verifyUpdaterSignature(f.content, f.signature, fixture(true, f.keyId).key), /artifact signature/);
});

test('rejects modified installer bytes', () => {
  const f = fixture();
  assert.throws(() => verifyUpdaterSignature(Buffer.concat([f.content, Buffer.from('tamper')]), f.signature, f.key), /artifact signature/);
});

test('rejects modified trusted comment', () => {
  const f = fixture();
  const modified = Buffer.from(Buffer.from(f.signature, 'base64').toString().replace('timestamp:1', 'timestamp:2')).toString('base64');
  assert.throws(() => verifyUpdaterSignature(f.content, modified, f.key), /trusted-comment signature/);
});

test('rejects modified signature bytes', () => {
  const f = fixture();
  const lines = Buffer.from(f.signature, 'base64').toString().split('\n');
  const signature = Buffer.from(lines[1], 'base64');
  signature[20] ^= 1;
  lines[1] = signature.toString('base64');
  assert.throws(() => verifyUpdaterSignature(f.content, Buffer.from(lines.join('\n')).toString('base64'), f.key), /artifact signature/);
});

test('rejects malformed base64 and truncated signatures', () => {
  const f = fixture();
  assert.throws(() => verifyUpdaterSignature(f.content, '!!!', f.key), /base64/);
  assert.throws(() => verifyUpdaterSignature(f.content, Buffer.from('truncated').toString('base64'), f.key), /envelope/);
});

test('verifies both release platforms and catches checksum drift', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'guard-signature-test-'));
  try {
    const f = fixture();
    const version = '0.0.1';
    const platforms = {};
    const files = new Map();
    for (const [kind, suffix] of [['nsis', 'x64-setup.exe'], ['msi', 'x64_en-US.msi']]) {
      const name = `Codex-Guard_${version}_${suffix}`;
      files.set(name, f.content);
      files.set(name + '.sig', Buffer.from(f.signature));
      platforms[`windows-x86_64-${kind}`] = { url: `https://github.com/Hessel2333/codex-guard/releases/download/v${version}/${name}`, signature: f.signature };
    }
    platforms['windows-x86_64'] = platforms['windows-x86_64-nsis'];
    files.set('latest.json', Buffer.from(JSON.stringify({ version, platforms })));
    for (const [name, bytes] of files) await writeFile(path.join(dir, name), bytes);
    const checksums = [...files].map(([name, bytes]) => `${createHash('sha256').update(bytes).digest('hex')}  ${name}`).join('\n');
    await writeFile(path.join(dir, 'SHA256SUMS.txt'), checksums);
    assert.equal((await verifyReleaseDirectory(dir, f.key, version)).signatures.length, 2);
    await assert.rejects(() => verifyReleaseDirectory(dir, f.key, '0.0.2'), /version mismatch/);
    await writeFile(path.join(dir, 'SHA256SUMS.txt'), checksums.replace(/^[a-f0-9]{64}/, '0'.repeat(64)));
    await assert.rejects(() => verifyReleaseDirectory(dir, f.key, version), /Checksum mismatch/);
    await writeFile(path.join(dir, 'Codex-Guard_0.0.1_x64-setup.exe.sig'), 'different');
    await assert.rejects(() => verifyReleaseDirectory(dir, f.key, version), /differs from sidecar/);
  } finally { await rm(dir, { recursive: true, force: true }); } // mkdtemp's own test directory only.
});
