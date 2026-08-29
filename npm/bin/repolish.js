#!/usr/bin/env node
// 启动器：把参数原样交给真正的二进制，退出码原样带回来。
//
// 退出码必须传回去,这一点不能有例外:repolish 的 `--min-score` 门禁、
// `verify` 的失败、`--base` 的基线错误,全靠退出码区分,而 CI 读的正是
// `npx repolish` 的退出码。
//
// 装的时候跳过了 scripts（`npm ci --ignore-scripts` 在 CI 里很常见）就
// 现下一次。第一次运行时多等几秒,好过甩一句「找不到二进制」。

'use strict';

const path = require('path');
const { spawnSync } = require('child_process');
const { download, isInstalled, BIN_PATH } = require('../install.js');

// 二进制在提示里怎么称呼自己,只有这里知道。
//
// `npx @asale/repolish check .` 解出来的那份跑完就没了,`repolish` 从来
// 没进过 PATH——所以印一句 ``Run `repolish check .``` 是把人送进 command
// not found。npm exec 有三种落点,三种说法:
//
// - npx 缓存(~/.npm/_npx/<hash>)：包名都是临时的,必须写全 `npx @asale/repolish`
// - 项目依赖 + `npx repolish`：短名字在这个项目里解析得到
// - 全局装：`repolish` 本来就在 PATH 上,不加前缀
function invokedAs() {
  if (__dirname.split(path.sep).includes('_npx')) return 'npx @asale/repolish';
  if (process.env.npm_command === 'exec') return 'npx repolish';
  return null;
}

function run() {
  const inv = invokedAs();
  const env = inv ? { ...process.env, REPOLISH_INVOKED_AS: inv } : process.env;
  const res = spawnSync(BIN_PATH, process.argv.slice(2), { stdio: 'inherit', env });
  if (res.error) {
    console.error(`repolish: could not run ${BIN_PATH}: ${res.error.message}`);
    process.exit(1);
  }
  // 被信号杀掉时 status 是 null。返回 1 而不是 0——
  // 一个被 SIGKILL 的检查在 CI 里绝不能读成「通过」。
  process.exit(res.status === null ? 1 : res.status);
}

if (isInstalled()) {
  run();
} else {
  download().then(run, (e) => {
    console.error(`repolish: ${e.message}`);
    process.exit(1);
  });
}
