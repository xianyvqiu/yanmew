import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {webcrypto} from 'node:crypto';
import {JSDOM} from 'jsdom';

const source = name => readFileSync(new URL(`../first-party/${name}`, import.meta.url), 'utf8');
const flush = async () => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
function host(coins = 100n, allowed = true) {
  const saved = new Map();
  const ledger = new Map();
  let run = '1';
  const sdk = {
    storage: {get: async key => structuredClone(saved.get(key) ?? null), set: async (key, data) => saved.set(key, structuredClone(data))},
    game: {ready: async () => {}, getContext: async () => ({gameRunId: run, walletPermissionGranted: allowed, permissions: [{permission:'wallet.modify', decision:allowed?'allowed':'denied'}]}), random: async () => .2},
    wallet: {getBalance: async () => ({coins: String(coins)}), changeCoins: async request => {
      if (ledger.has(request.request_id)) return {balance: String(coins), duplicate: true};
      if (!allowed || request.game_run_id !== run) throw {code:'PLUGIN_PERMISSION_DENIED',message:'权限被拒绝'};
      if (coins + BigInt(request.delta) < 0n) throw {code:'INSUFFICIENT_COINS',message:'余额不足'};
      coins += BigInt(request.delta); ledger.set(request.request_id, request);
      return {balance: String(coins), duplicate:false};
    }},
  };
  return {sdk, saved, ledger, balance: () => coins, reopen: () => {run = String(Number(run)+1);}, allow: value => {allowed = value;}};
}
async function load(name, h) {
  const dom = new JSDOM(source(`${name}/ui/index.html`), {url:'https://plugin.invalid'});
  const win = dom.window;
  win.yanmew = h.sdk;
  win.HTMLElement.prototype.animate = function () { return {set onfinish(callback) { queueMicrotask(callback); }}; };
  const context = vm.createContext({window: win, document: win.document, console, crypto: webcrypto, structuredClone,
    setTimeout: callback => {queueMicrotask(callback); return 1;}, clearTimeout: () => {},
    setInterval: () => 1, clearInterval: () => {}, requestAnimationFrame: () => 1, cancelAnimationFrame: () => {}, performance});
  vm.runInContext(source('shared/runtime.js'), context);
  context.FirstPartyRuntime = win.FirstPartyRuntime;
  if (name === 'my-home') vm.runInContext(source('my-home/ui/fishing.js'), context);
  vm.runInContext(source(`${name}/ui/game.js`), context);
  await flush();
  return {context, win, run: code => vm.runInContext(code, context), close: () => dom.window.close()};
}
test('扣款已提交但进度写入失败：重开核对同一请求，不二次扣款', async () => {
  const h = host(); const game = await load('my-home', h);
  const Runtime = game.win.FirstPartyRuntime;
  const r = new Runtime({inventory:{}}); await r.initialize();
  const write = h.sdk.storage.set; let writes = 0;
  h.sdk.storage.set = async (...args) => { if (++writes === 2) throw new Error('保存中断'); return write(...args); };
  await assert.rejects(r.commit({inventory:{seed:1}}, -5, '购买种子'));
  assert.equal(h.balance(), 95n); assert.equal(h.ledger.size, 1);
  h.sdk.storage.set = write; h.reopen();
  const recovered = new Runtime({inventory:{}}); await recovered.initialize();
  assert.deepEqual(recovered.progress, {inventory:{seed:1}});
  assert.equal(h.balance(),95n); assert.equal(h.ledger.size,1); game.close();
});
test('旧运行未扣款：不重放请求、不赠送购买结果', async () => {
  const h=host(); const game=await load('my-home',h); const Runtime=game.win.FirstPartyRuntime;
  h.saved.set('state.main', {version:1,progress:{inventory:{}},pending:{request:{request_id:'old',game_run_id:'1',delta:-10,reason:'购买'},after:{inventory:{gift:1}}}});
  h.reopen(); const r=new Runtime({inventory:{}}); await r.initialize();
  assert.deepEqual(r.progress,{inventory:{}}); assert.equal(h.balance(),100n); assert.equal(h.ledger.size,0); game.close();
});
test('余额不足不发布新进度，保存请求被清除',async()=>{
  const h=host(1n);const game=await load('my-home',h);const r=game.run('runtime');
  await assert.rejects(r.commit({...r.progress,affinity:100},-5,'购买'));
  assert.equal(r.progress.affinity,0);assert.equal(h.saved.get('state.main').pending,null);game.close();
});
test('剪刀石头布：真实钱包先结算，每插件次数持久化',async()=>{
  const h=host();h.sdk.game.random=async()=>.4;const game=await load('rock-paper-scissors',h);
  await game.run("play('rock')");
  assert.equal(h.balance(),105n);assert.equal(h.ledger.size,1);
  assert.equal(h.saved.get('state.main').progress.usedToday,1);assert.equal(game.run('state.balance'),105n);
  h.reopen();game.close();const again=await load('rock-paper-scissors',h);assert.equal(again.run('state.usedToday'),1);again.close();
});
test('未授权可纯玩，不产生原型初始喵币；每天20局独立限制',async()=>{
  const h=host(0n,false);const game=await load('rock-paper-scissors',h);
  for(let i=0;i<20;i++) await game.run("play('paper')");
  await game.run("play('paper')");assert.equal(h.balance(),0n);assert.equal(h.ledger.size,0);assert.equal(game.run('state.usedToday'),20);game.close();
});
test('跨本地日期恢复次数，记录完整年月日',async()=>{
  const h=host(0n,false);h.saved.set('state.main',{version:1,progress:{day:'2020-01-01',usedToday:20,walletMode:false},pending:null});
  const game=await load('dice',h);await game.run('roll()');
  const p=h.saved.get('state.main').progress;assert.equal(p.usedToday,1);assert.match(p.day,/^\d{4}-\d{2}-\d{2}$/);game.close();
});
test('骰子胜利与幸运6只写一笔净奖励，关闭也不会漏发',async()=>{
  const h=host();let draws=[.99,.7];h.sdk.game.random=async()=>draws.shift();const game=await load('dice',h);
  await game.run('roll()');assert.equal(h.balance(),105n);assert.equal(h.ledger.size,1);assert.equal([...h.ledger.values()][0].delta,5);assert.equal(game.run('state.balance'),105n);game.close();
});
test('接星星中途关闭：保留8币入场费和次数，不补发奖励',async()=>{
  const h=host();const game=await load('star-catcher',h);await game.run('start()');
  assert.equal(h.balance(),92n);assert.equal(game.run('state.playing'),true);assert.equal(h.saved.get('state.main').progress.usedToday,1);
  game.close();h.reopen();const again=await load('star-catcher',h);
  assert.equal(h.balance(),92n);assert.equal(h.ledger.size,1);assert.equal(h.saved.get('state.main').progress.activeRound,null);again.close();
});
test('接星星结束按实际得分加币，结算中禁止开始新局',async()=>{
  const h=host();const game=await load('star-catcher',h);await game.run('start()');game.run('score = 7');
  const finish=game.run('finish()');await game.run('start()');await finish;
  assert.equal(h.balance(),99n);assert.equal(h.ledger.size,2);assert.equal(h.saved.get('state.main').progress.usedToday,1);game.close();
});
test('我的家拒绝纯玩；大额钱包精度不丢失，购买保存库存且不重复',async()=>{
  const denied=host(100n,false);const refused=await load('my-home',denied);await refused.run("buySeed('白菜种子')");assert.equal(denied.ledger.size,0);refused.close();
  const h=host(9007199254740993n);const game=await load('my-home',h);
  await Promise.all([game.run("buySeed('白菜种子')"),game.run("buySeed('白菜种子')")]);
  assert.equal(h.balance(),9007199254740988n);assert.equal(h.saved.get('state.main').progress.inventory['白菜种子'],1);assert.equal(h.ledger.size,1);game.close();
});
test('菜品按原标价卖出，私有熟练度保存且不修改桌宠亲密度',async()=>{
  const h=host();const game=await load('my-home',h);game.run("state.inventory['番茄炒蛋'] = 1; state.inventory['番茄'] = 1");
  await game.run("sellItem('番茄炒蛋')");assert.equal(h.balance(),120n);assert.equal(h.saved.get('state.main').progress.inventory['番茄炒蛋'],undefined);
  await game.run("sendGift('番茄')");assert.ok(h.saved.get('state.main').progress.affinity>=1);assert.equal(h.ledger.size,1);game.close();
});
test('宰杀弹窗取消后再确认，不累积旧回调或重复奖励',async()=>{
  const h=host();const game=await load('my-home',h);game.run("state.inventory['鸡_养殖量'] = 2; killAnimal('鸡'); closeModal(document.querySelector('#confirm-modal')); killAnimal('鸡')");
  game.win.document.querySelector('[data-confirm-yes]').click();await flush();
  const p=h.saved.get('state.main').progress;assert.equal(p.inventory['鸡_养殖量'],1);assert.equal(p.inventory['鸡肉'],2);game.close();
});

test('互动钓鱼先落盘消耗，再收鱼；重复完成不重复奖励，重开保留鱼获',async()=>{
  const h=host();const game=await load('my-home',h);let challenge;
  game.win.HomeFishing={...game.win.HomeFishing,open:options=>{challenge=options;}};
  game.run('state.affinity=30');await game.run("doFish('low')");
  assert.ok(challenge);assert.equal(h.balance(),99n);assert.equal(h.ledger.size,1);
  let p=h.saved.get('state.main').progress;
  assert.equal(p.fishing.dailyCount,1);assert.equal(p.fishing.rodDurability,79);assert.equal(p.fishing.fishGrams,0);
  await game.run("doFish('high')");assert.equal(h.balance(),99n);
  assert.equal(await challenge.onResult(true),true);assert.equal(await challenge.onResult(true),true);
  p=h.saved.get('state.main').progress;assert.equal(p.fishing.fishGrams,challenge.grams);assert.equal(h.ledger.size,1);
  challenge.onClose();game.close();h.reopen();const reopened=await load('my-home',h);
  assert.equal(reopened.run('state.fishing.fishGrams'),challenge.grams);assert.equal(reopened.run('state.fishing.dailyCount'),1);reopened.close();
});

test('互动钓鱼放弃或关闭后消耗保留，保存鱼获失败可重试且不再扣费',async()=>{
  const h=host();const game=await load('my-home',h);let challenge;
  game.win.HomeFishing={...game.win.HomeFishing,open:options=>{challenge=options;}};game.run('state.affinity=30');
  await game.run("doFish('high')");await challenge.onResult(false);challenge.onClose();
  assert.equal(h.balance(),98n);assert.equal(h.saved.get('state.main').progress.fishing.fishGrams,0);
  game.close();h.reopen();const reopened=await load('my-home',h);
  assert.equal(reopened.run('state.fishing.dailyCount'),1);assert.equal(reopened.run('state.fishing.rodDurability'),79);
  reopened.win.HomeFishing={...reopened.win.HomeFishing,open:options=>{challenge=options;}};await reopened.run("doFish('low')");
  const write=h.sdk.storage.set;let reject=true;h.sdk.storage.set=async(key,data)=>{if(reject&&data.progress?.fishing?.fishGrams>0){reject=false;throw new Error('保存中断');}return write(key,data);};
  assert.equal(await challenge.onResult(true),false);assert.equal(h.balance(),97n);
  assert.equal(await challenge.onResult(true),true);assert.equal(h.balance(),97n);
  assert.equal(h.saved.get('state.main').progress.fishing.fishGrams,challenge.grams);assert.equal(h.ledger.size,2);reopened.close();
});
