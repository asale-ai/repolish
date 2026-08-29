// npm 壳的测试。不联网。
//
// 这层壳的两个危险点都是「安静地做错事」：解错归档得到一个跑不起来的文件，
// 或者把退出码吞掉让 CI 变绿。两者都值得盯住。

'use strict';

const assert = require('assert');
const path = require('path');

const mod = require('./install.js');

// ── 版本对齐 ──────────────────────────────────────────────────────────
//
// npm 上装到 0.4.0 却跑起来是 0.3.0 的二进制，是最难查的一类问题。
// 版本号是**下载 URL 的一部分**，所以它必须等于 workspace 的版本。
{
  const fs = require('fs');
  const cargo = fs.readFileSync(path.join(__dirname, '..', 'Cargo.toml'), 'utf8');
  const m = cargo.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
  assert.ok(m, 'could not read the workspace version out of Cargo.toml');
  assert.strictEqual(
    mod.VERSION,
    m[1],
    `npm/package.json says ${mod.VERSION} but Cargo.toml says ${m[1]}; ` +
      'the version is part of the release download URL, so they cannot drift'
  );
}

// ── 包名与发布可见性 ──────────────────────────────────────────────────
//
// 作用域包默认是 restricted。忘了 public，`npm publish` 会成功，而所有人
// `npx` 得到 404 —— 一个看起来发出去了、实际没人装得上的版本。
{
  const pkg = require('./package.json');
  assert.strictEqual(pkg.name, '@asale/repolish');
  assert.strictEqual(
    pkg.publishConfig && pkg.publishConfig.access,
    'public',
    'a scoped package without publishConfig.access = public publishes as restricted'
  );
  // bin 名不带作用域：使用者敲的是 `repolish`，不是 `@asale/repolish`
  assert.deepStrictEqual(Object.keys(pkg.bin), ['repolish']);
}

// ── tar 解包 ──────────────────────────────────────────────────────────
//
// 自己读 tar 头是为了不引依赖。代价是这段代码没有上游在维护，
// 所以它自己要有测试。
{
  const { extractFromTar } = load();
  const tar = makeTar([
    ['repolish-v9.9.9-x/README.md', Buffer.from('not the binary')],
    ['repolish-v9.9.9-x/repolish', Buffer.from('#!/bin/sh\necho hi\n')],
  ]);
  const got = extractFromTar(tar, 'repolish');
  assert.ok(got, 'the binary was not found in the archive');
  assert.strictEqual(got.toString(), '#!/bin/sh\necho hi\n');
  assert.strictEqual(extractFromTar(tar, 'nothing-like-this'), null);
}

// ── zip 解包 ──────────────────────────────────────────────────────────
{
  const { extractFromZip } = load();
  const body = Buffer.from('MZ fake windows binary');
  const zip = makeStoredZip('repolish-v9.9.9-x/repolish.exe', body);
  assert.strictEqual(extractFromZip(zip, 'repolish.exe').toString(), body.toString());
}

// ── 退出码 ────────────────────────────────────────────────────────────
//
// `--min-score` 门禁、`verify` 的失败、`--base` 的基线错误全靠退出码区分，
// 而 CI 读的正是 `npx repolish` 的退出码。吞掉一个非零码，等于让门禁失效。
{
  const launcher = path.join(__dirname, 'bin', 'repolish.js');
  const src = require('fs').readFileSync(launcher, 'utf8');
  assert.ok(
    /process\.exit\(res\.status === null \? 1 : res\.status\)/.test(src),
    'the launcher must pass the real exit code through, and treat "killed by a signal" as failure'
  );
  assert.ok(
    /stdio: 'inherit'/.test(src),
    'stdout and stderr must pass straight through; --format json must stay parseable'
  );
}

console.log('npm shim: ok');

// ── 帮手 ──────────────────────────────────────────────────────────────

/// install.js 只导出运行时要用的东西。解包函数是内部的，
/// 测试里用 require 拿不到——重新 eval 一份，比为了测试把内部函数导出去好。
function load() {
  const fs = require('fs');
  const src = fs.readFileSync(path.join(__dirname, 'install.js'), 'utf8');
  const sandbox = { module: { exports: {} }, require, process, __dirname, console, Buffer };
  sandbox.exports = sandbox.module.exports;
  const fn = new Function(
    'module',
    'exports',
    'require',
    '__dirname',
    `${src}\nmodule.exports.__test = { extractFromTar, extractFromZip, target };`
  );
  fn(sandbox.module, sandbox.exports, require, __dirname);
  return sandbox.module.exports.__test;
}

function makeTar(entries) {
  const blocks = [];
  for (const [name, body] of entries) {
    const header = Buffer.alloc(512);
    header.write(name, 0, 100, 'utf8');
    header.write('0000644\0', 100);
    header.write('0000000\0', 108);
    header.write('0000000\0', 116);
    header.write(body.length.toString(8).padStart(11, '0') + '\0', 124);
    header.write('00000000000\0', 136);
    header.write('        ', 148); // 校验和字段：我们的读取器不验它
    header.write('0', 156);
    let sum = 0;
    for (const b of header) sum += b;
    header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148);
    blocks.push(header, pad(body));
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

function pad(buf) {
  const size = Math.ceil(buf.length / 512) * 512;
  const out = Buffer.alloc(size);
  buf.copy(out);
  return out;
}

/// 不压缩的 zip（method 0）。测的是「找得到那个条目」，不是 inflate。
function makeStoredZip(name, body) {
  const nameBuf = Buffer.from(name, 'utf8');
  const header = Buffer.alloc(30);
  header.writeUInt32LE(0x04034b50, 0);
  header.writeUInt16LE(20, 4);
  header.writeUInt16LE(0, 6);
  header.writeUInt16LE(0, 8); // stored
  header.writeUInt32LE(0, 14);
  header.writeUInt32LE(body.length, 18);
  header.writeUInt32LE(body.length, 22);
  header.writeUInt16LE(nameBuf.length, 26);
  header.writeUInt16LE(0, 28);
  return Buffer.concat([header, nameBuf, body]);
}
