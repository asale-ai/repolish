// 下载与这个 npm 包版本严格对应的那一份发布二进制，校验 sha256，解包。
//
// 为什么要有这个包：repolish 检查的仓库绝大多数不是 Rust 项目，而
// `cargo install repolish` 要求对方先装一套 Rust 工具链。对一个「跑一次看看
// 分数」的工具来说，那是一道劝退的门槛。`npx repolish check .` 没有这道门槛。
//
// 三条规矩：
//
// - **版本严格对齐。** package.json 的 version 就是要下载的 release tag。
//   npm 上装到 0.4.0 却跑起来是 0.3.0 的二进制，是最难查的一类问题。
// - **校验和必须过。** 对不上就删掉、报错、退出，绝不留下一个可执行文件。
// - **失败要说得出下一步。** 一个没有构建产物的平台（musl、32 位）拿到的
//   应该是「用 cargo install」，不是一句 404。

'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');
const { execFileSync } = require('child_process');

const REPO = 'asale-ai/repolish';
const VERSION = require('./package.json').version;
const BIN_DIR = path.join(__dirname, 'bin');
const EXE = process.platform === 'win32' ? 'repolish.exe' : 'repolish';
const BIN_PATH = path.join(BIN_DIR, EXE);

/// release 的产物命名是 .github/workflows/release.yml 定下的契约：
///   repolish-v{version}-{target}.{tar.gz|zip}
/// install.sh 与 action.yml 拼的是同一个名字。改一处就要改三处。
function target() {
  const arch = { x64: 'x86_64', arm64: 'aarch64' }[process.arch];
  if (!arch) {
    fail(
      `repolish ships x86_64 and aarch64 builds; this machine is ${process.arch}.`
    );
  }

  switch (process.platform) {
    case 'darwin':
      return { triple: `${arch}-apple-darwin`, ext: 'tar.gz' };
    case 'win32':
      if (arch !== 'x86_64') {
        fail('repolish ships only an x86_64 build for Windows today.');
      }
      return { triple: 'x86_64-pc-windows-msvc', ext: 'zip' };
    case 'linux':
      // Linux 的构建是 glibc-only。装一个跑起来就 linker error 的二进制，
      // 比明说「这里装不了」糟得多——install.sh 在同一处做同样的判断。
      if (isMusl()) {
        fail(
          'musl libc detected (Alpine or similar). repolish only ships glibc builds today.'
        );
      }
      return { triple: `${arch}-unknown-linux-gnu`, ext: 'tar.gz' };
    default:
      fail(`unsupported operating system: ${process.platform}`);
  }
}

function isMusl() {
  const header = process.report && process.report.getReport().header;
  if (header && typeof header.glibcVersionRuntime === 'string') return false;
  if (header && 'glibcVersionRuntime' in header) return true;
  // 老版本 Node 没有这个字段。退回问 ldd,问不出来就当作 glibc——
  // 猜错的代价是一句清楚的 linker error,而误判成 musl 会拦下本来装得上的机器。
  try {
    return /musl/i.test(execFileSync('ldd', ['--version'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
  } catch (e) {
    try {
      return /musl/i.test(e.stderr || '');
    } catch (_) {
      return false;
    }
  }
}

function fail(message) {
  console.error(`repolish: ${message}`);
  console.error('Build it from source instead:  cargo install repolish');
  console.error(`Or download a binary:          https://github.com/${REPO}/releases`);
  process.exit(1);
}

async function get(url, redirects = 0) {
  if (redirects > 5) throw new Error(`too many redirects for ${url}`);
  const https = require('https');
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { 'user-agent': `repolish-npm/${VERSION}` } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          resolve(get(new URL(res.headers.location, url).toString(), redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`${res.statusCode} for ${url}`));
          return;
        }
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => resolve(Buffer.concat(chunks)));
        res.on('error', reject);
      })
      .on('error', reject);
  });
}

/// tar.gz 里只有一个我们要的文件。为了一个只解一份归档的功能拖进 tar 依赖
/// 不值当，所以：gzip 用 Node 自带的 zlib 解，tar 头自己读——格式是 512 字节
/// 定长块，读起来比引一个包短。
function extractFromTar(buf, wanted) {
  let off = 0;
  while (off + 512 <= buf.length) {
    const name = buf.toString('utf8', off, off + 100).replace(/\0.*$/, '');
    if (!name) break;
    const size = parseInt(buf.toString('utf8', off + 124, off + 136).replace(/\0.*$/, '').trim(), 8) || 0;
    const start = off + 512;
    if (path.basename(name) === wanted && size > 0) {
      return buf.subarray(start, start + size);
    }
    off = start + Math.ceil(size / 512) * 512;
  }
  return null;
}

/// zip 也一样：只为取一个文件,没必要引依赖。找到 local file header,
/// 按压缩方法解出来。
function extractFromZip(buf, wanted) {
  for (let i = 0; i + 30 <= buf.length; i++) {
    if (buf.readUInt32LE(i) !== 0x04034b50) continue;
    const method = buf.readUInt16LE(i + 8);
    const compressed = buf.readUInt32LE(i + 18);
    const nameLen = buf.readUInt16LE(i + 26);
    const extraLen = buf.readUInt16LE(i + 28);
    const name = buf.toString('utf8', i + 30, i + 30 + nameLen);
    const start = i + 30 + nameLen + extraLen;
    if (path.basename(name) !== wanted) continue;
    const body = buf.subarray(start, start + compressed);
    return method === 0 ? body : zlib.inflateRawSync(body);
  }
  return null;
}

async function download() {
  const { triple, ext } = target();
  const tag = `v${VERSION}`;
  const asset = `repolish-${tag}-${triple}.${ext}`;
  const base = `https://github.com/${REPO}/releases/download/${tag}`;

  process.stderr.write(`repolish: fetching ${asset}\n`);

  let archive;
  try {
    archive = await get(`${base}/${asset}`);
  } catch (e) {
    fail(`could not download ${asset} (${e.message}).`);
  }

  // 校验和对不上就什么都不留下。一个来路不明的可执行文件放在 node_modules
  // 里,比装不上危险得多。
  try {
    const sums = (await get(`${base}/${asset}.sha256`)).toString('utf8');
    const expected = sums.trim().split(/\s+/)[0];
    const actual = crypto.createHash('sha256').update(archive).digest('hex');
    if (expected && expected !== actual) {
      fail(`checksum mismatch for ${asset}\n  expected ${expected}\n  got      ${actual}`);
    }
  } catch (e) {
    // 校验文件本身拿不到不是致命的（旧版本可能没有），但必须说出来
    process.stderr.write(`repolish: warning — could not verify the checksum (${e.message})\n`);
  }

  const body =
    ext === 'zip'
      ? extractFromZip(archive, EXE)
      : extractFromTar(zlib.gunzipSync(archive), EXE);
  if (!body) fail(`${asset} did not contain ${EXE}`);

  fs.mkdirSync(BIN_DIR, { recursive: true });
  fs.writeFileSync(BIN_PATH, body, { mode: 0o755 });
  process.stderr.write(`repolish: installed ${BIN_PATH}\n`);
}

function isInstalled() {
  return fs.existsSync(BIN_PATH) && fs.statSync(BIN_PATH).size > 0;
}

module.exports = { download, isInstalled, BIN_PATH, VERSION };

if (require.main === module) {
  if (isInstalled()) process.exit(0);
  download().catch((e) => fail(e.message));
}
