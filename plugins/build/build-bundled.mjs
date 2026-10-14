// 第一方源码使用同一个宿主校验器打包；不运行游戏，也不读取开发数据库。
import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const catalogue = JSON.parse(readFileSync(join(root, 'plugins/bundled-games.json'), 'utf8'));
const target = join(root, 'resources/plugins');
mkdirSync(target, {recursive: true});
execFileSync('cargo', ['build', '--manifest-path', join(root, 'src-tauri/Cargo.toml'), '--bin', 'yanmew-plugin'], {stdio: 'inherit'});
const packer = join(root, 'src-tauri/target/debug', process.platform === 'win32' ? 'yanmew-plugin.exe' : 'yanmew-plugin');
for (const filename of catalogue.packages) {
  if (!/^[a-z0-9-]+\.yanmew-game$/.test(filename)) throw new Error('随包清单包含不安全的文件名');
  const source = join(root, 'plugins/first-party', filename.replace('.yanmew-game', ''));
  copyFileSync(join(root, 'plugins/first-party/shared/runtime.js'), join(source, 'ui/runtime.js'));
  // 临时目标已解析并限制在资源目录；CLI 不覆盖已有输出，验证成功才替换发布文件。
  const temporary = join(target, filename.replace('.yanmew-game', '.tmp.yanmew-game'));
  if (existsSync(temporary)) rmSync(temporary);
  try {
    execFileSync(packer, ['pack', source, temporary], {stdio: 'inherit'});
    execFileSync(packer, ['validate', temporary], {stdio: 'inherit'});
    renameSync(temporary, join(target, filename));
  } finally { if (existsSync(temporary)) rmSync(temporary); }
}
copyFileSync(join(root, 'plugins/bundled-games.json'), join(target, 'bundled-games.json'));
