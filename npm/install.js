// 下载与这个 npm 包版本严格对应的那一份发布二进制，校验 sha256，解包。
//
// 为什么要有这个包：repolish 检查的仓库绝大多数不是 Rust 项目，而
// `cargo install repolish` 要求对方先装一套 Rust 工具链。对一个「跑一次看看
// 分数」的工具来说，那是一道劝退的门槛。`npx @asale/repolish` 没有这道门槛。
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

/// 一次 HTTP 失败是「服务器说了不」，还是「线路抖了一下」。
///
/// 这两件事必须分开，因为它们的正确反应相反：404 说明这个东西确实不存在，
/// 重试一百次也一样；ETIMEDOUT / ECONNRESET 只说明这一次没通，而 GitHub 的
/// release 下载偶尔就是会这样。
function isRetryable(err) {
  if (err && typeof err.status === 'number') {
    // 4xx 是服务器的结论，别浪费时间；5xx 和 408/429 值得再试一次
    return err.status >= 500 || err.status === 408 || err.status === 429;
  }
  return true; // 连接层的错误，一律重试
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/// 单次请求的超时。
///
/// **必须显式设。** 默认没有超时，一条走不通的线路要等操作系统的 connect
/// 超时,在 macOS 上是四十秒左右;乘上三次重试就是两分钟的沉默。实测过:
/// 一次 `npm install` 在这里挂了 124 秒才报错。
const TIMEOUT_MS = 20000;

async function once(url, redirects = 0) {
  if (redirects > 5) throw new Error(`too many redirects for ${url}`);
  const https = require('https');
  return new Promise((resolve, reject) => {
    const req = https.get(
      url,
      { headers: { 'user-agent': `repolish-npm/${VERSION}` }, timeout: TIMEOUT_MS },
      (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          resolve(once(new URL(res.headers.location, url).toString(), redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          const e = new Error(`${res.statusCode} for ${url}`);
          e.status = res.statusCode;
          reject(e);
          return;
        }
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => resolve(Buffer.concat(chunks)));
        res.on('error', reject);
      }
    );
    // timeout 事件不会自己中断请求——不 destroy 的话它就一直挂着
    req.on('timeout', () => {
      req.destroy(new Error(`timed out after ${TIMEOUT_MS / 1000}s`));
    });
    req.on('error', reject);
  });
}

/// 重试三次，退避 1s / 2s。
///
/// 装东西是使用者做的第一件事，而 `npx` 那一下没有「再试一次」的按钮——
/// 第一次超时就等于这个工具在他那里根本装不上。
///
/// 每次重试都说出来。一个安静地卡着的安装过程，使用者唯一能做的判断是
/// 「它是不是死了」,而那个判断通常以 Ctrl-C 结束。
async function get(url, attempts = 3) {
  let last;
  for (let i = 0; i < attempts; i++) {
    try {
      return await once(url);
    } catch (e) {
      last = e;
      if (!isRetryable(e) || i === attempts - 1) break;
      const wait = 1000 * 2 ** i;
      process.stderr.write(
        `repolish: ${e.message} — retrying in ${wait / 1000}s (${i + 2}/${attempts})\n`
      );
      await sleep(wait);
    }
  }
  throw last;
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
  //
  // **拿不到校验文件与拿到一个不匹配的，反应不同；但拿不到的两种原因,
  // 反应也不同。** 404 是「这个 release 确实没有 .sha256」——旧版本就是这样,
  // 为此拒绝安装说不过去。而 ECONNRESET 只是这一次没通,它对那个文件存不存在
  // 一无所知,把它当成「没有校验和」就是让一次网络抖动把校验悄悄关掉。
  // 这一条是实测撞出来的：归档下下来了,紧接着那个几十字节的 .sha256 被
  // 重置了连接,于是一个未经验证的可执行文件带着一行警告落了盘。
  let sums;
  try {
    sums = (await get(`${base}/${asset}.sha256`)).toString('utf8');
  } catch (e) {
    if (e && e.status === 404) {
      process.stderr.write(
        `repolish: warning — ${asset}.sha256 is not in this release; installing unverified\n`
      );
    } else {
      fail(
        `could not fetch ${asset}.sha256 (${e.message}).\n` +
          '  The archive downloaded but could not be verified, so nothing was installed.\n' +
          '  This is usually transient — run it again.'
      );
    }
  }
  if (sums) {
    const expected = sums.trim().split(/\s+/)[0];
    const actual = crypto.createHash('sha256').update(archive).digest('hex');
    if (expected && expected !== actual) {
      fail(`checksum mismatch for ${asset}\n  expected ${expected}\n  got      ${actual}`);
    }
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
