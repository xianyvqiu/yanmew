import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import { webcrypto } from 'node:crypto';
import { JSDOM } from 'jsdom';
const source = name => readFileSync(new URL(`../first-party/${name}`, import.meta.url), 'utf8');
function engine(name, key) { const window = {}; vm.runInNewContext(source(`${name}/ui/engine.js`), {window,structuredClone,Math}); return window[key]; }
function host(coins=1000n,allowed=true) {
  const saved=new Map(),ledger=new Map();let run='1';
  return {saved,ledger,balance:()=>coins,reopen:()=>{run=String(Number(run)+1);},sdk:{storage:{get:async key=>structuredClone(saved.get(key)??null),set:async(key,value)=>saved.set(key,structuredClone(value))},game:{ready:async()=>{},getContext:async()=>({gameRunId:run,walletPermissionGranted:allowed,permissions:[{permission:'wallet.modify',decision:allowed?'allowed':'denied'}]})},wallet:{getBalance:async()=>({coins:String(coins)}),changeCoins:async request=>{
    if(ledger.has(request.request_id))return{balance:String(coins),duplicate:true};
    if(!allowed||request.game_run_id!==run)throw{code:'PLUGIN_PERMISSION_DENIED'};
    if(coins+BigInt(request.delta)<0n)throw{code:'INSUFFICIENT_COINS'};
    coins+=BigInt(request.delta);ledger.set(request.request_id,request);return{balance:String(coins),duplicate:false};
  }}}};
}
async function load(name,h) {
  const dom=new JSDOM(source(`${name}/ui/index.html`),{url:'https://plugin.invalid'}),window=dom.window;window.yanmew=h.sdk;
  const context=vm.createContext({window,document:window.document,console,crypto:webcrypto,structuredClone,Math,Date,setInterval:()=>1,clearInterval:()=>{},setTimeout,clearTimeout});
  for(const file of ['shared/runtime.js','shared/classic-runtime.js',`${name}/ui/engine.js`,`${name}/ui/game.js`])vm.runInContext(source(file),context);
  await new Promise(resolve=>setImmediate(resolve));
  return {run:script=>vm.runInContext(script,context),window,close:()=>window.close()};
}
test('扫雷三档规格、首击安全与真实地雷数量',()=>{
  const r=engine('minesweeper','MinesweeperRules');
  for(const [level,rows,columns,mines] of [['easy',8,8,10],['medium',16,16,40],['hard',16,30,99]]){
    const board=r.open(r.create(level),0,()=>.37);
    assert.equal(board.rows,rows);assert.equal(board.columns,columns);assert.equal(board.cells.filter(c=>c.mine).length,mines);assert.equal(board.cells[0].mine,false);assert.equal(board.cells[0].open,true);
  }
});
test('扫雷右键循环、旗子阻止翻开、正确双击展开和踩雷失败',()=>{
  const r=engine('minesweeper','MinesweeperRules');let b=r.open(r.create('easy'),0,()=>.4);
  const hidden=b.cells.findIndex(c=>!c.open);b=r.mark(b,hidden);assert.equal(b.cells[hidden].mark,1);assert.equal(r.open(b,hidden).cells[hidden].open,false);b=r.mark(b,hidden);assert.equal(b.cells[hidden].mark,2);b=r.mark(b,hidden);assert.equal(b.cells[hidden].mark,0);
  const mine=b.cells.findIndex(c=>c.mine);assert.equal(r.open(b,mine).status,'lost');
  b.cells.forEach((cell,index)=>{if(cell.mine)b=r.mark(b,index);});
  for(let i=0;i<b.cells.length;i++)if(b.cells[i].open&&b.cells[i].number)b=r.chord(b,i);
  assert.notEqual(b.status,'lost');
  for(let i=0;i<b.cells.length;i++)if(!b.cells[i].mine)b=r.open(b,i);
  assert.equal(b.status,'won');assert.equal(b.cells.filter(c=>c.mark===1).length,10);
});
test('2048单次合并、四向移动、不移动不新增与无法移动判定',()=>{
  const r=engine('2048','Twenty48Rules'),board=[2,2,2,2,...Array(12).fill(0)];
  const moved=r.shift(board,'left');assert.deepEqual(Array.from(moved.board.slice(0,4)),[4,4,0,0]);assert.equal(moved.score,8);assert.equal(board[0],2);
  assert.deepEqual(Array.from(r.shift(board,'right').board.slice(0,4)),[0,0,4,4]);
  assert.equal(r.shift([2,0,0,0,...Array(12).fill(0)],'left').changed,false);
  assert.equal(r.shift([2,0,0,0,2,...Array(11).fill(0)],'down').board[12],4);
  assert.equal(r.canMove([2,4,2,4,4,2,4,2,2,4,2,4,4,2,4,2]),false);
});
test('2048奖励仅按最高档位一次发放',()=>{
  const r=engine('2048','Twenty48Rules');for(const [tile,reward]of[[128,0],[256,10],[512,25],[1024,50],[2048,200],[4096,200]])assert.equal(r.reward(tile),reward);
});
test('扫雷赚钱入场、通关双倍奖励，重开不重复收费或领奖',async()=>{
  const h=host();let game=await load('minesweeper',h);await game.run("start('earn')");assert.equal(h.balance(),995n);
  h.reopen();game.close();game=await load('minesweeper',h);assert.equal(h.balance(),995n);assert.equal(game.run('runtime.progress.current.settled'),false);
  await game.run("move('open',0)");
  await game.run("(async()=>{for(let i=0;i<runtime.progress.current.cells.length;i++){if(!runtime.progress.current.cells[i].mine)await move('open',i);}})()");
  assert.equal(h.balance(),1005n);assert.equal(h.ledger.size,2);assert.equal(game.run('runtime.progress.current.settled'),true);
  await game.run("move('open',0)");assert.equal(h.balance(),1005n);game.close();
});
test('每款赚钱局每天最多5次，跨日恢复；余额不足不占次数',async()=>{
  for(const name of ['minesweeper','2048']){
    const h=host(),game=await load(name,h);
    for(let i=0;i<5;i++){await game.run("start('earn')");await game.run("get('end').onclick()");}
    const balance=h.balance();await game.run("start('earn')");assert.equal(h.balance(),balance);assert.equal(game.run('runtime.usedToday'),5);
    await game.run("runtime.save({...runtime.progress,day:'2020-01-01'})");await game.run("start('earn')");assert.equal(game.run('runtime.usedToday'),1);game.close();
    const poor=host(0n),empty=await load(name,poor);await empty.run("start('earn')");assert.equal(empty.run('runtime.usedToday'),0);assert.equal(poor.ledger.size,0);empty.close();
  }
});
test('免费游玩不需要钱包权限，不计赚钱次数也不产生收支',async()=>{
  for(const name of ['minesweeper','2048']){
    const h=host(0n,false),game=await load(name,h);await game.run("start('play')");await game.run("get('end').onclick()");assert.equal(h.balance(),0n);assert.equal(h.ledger.size,0);assert.equal(game.run('runtime.usedToday'),0);game.close();
  }
});
test('2048各奖励档位实际结算、结束后重复点击不再领奖',async()=>{
  for(const[highest,reward]of[[256,10],[512,25],[1024,50],[2048,200]]){
    const h=host(),game=await load('2048',h);await game.run("start('earn')");await game.run(`runtime.save({...runtime.progress,current:{...runtime.progress.current,highest:${highest}}})`);await game.run("get('end').onclick()");assert.equal(h.balance(),1000n-20n+BigInt(reward));const balance=h.balance();await game.run("get('end').onclick()");assert.equal(h.balance(),balance);game.close();
  }
});
test('帮助和扫雷胜负弹窗只展示，不额外扣币或结算',async()=>{
 const h=host(),game=await load('minesweeper',h);game.window.document.getElementById('help').click();
 assert.match(game.window.document.querySelector('.classic-dialog').textContent,/首击安全/);assert.equal(h.ledger.size,0);
 await game.run("start('earn')");await game.run("move('open',0)");const mine=game.run('runtime.progress.current.cells.findIndex(c=>c.mine)');await game.run(`move('open',${mine})`);
 assert.equal(game.window.document.querySelector('.classic-dialog').dataset.kind,'lost');assert.equal(h.balance(),995n);const count=h.ledger.size;game.window.document.querySelector('.classic-dialog button').click();assert.equal(h.ledger.size,count);game.close();
});
test('2048达成目标只庆祝不提前领奖，棋盘无路后按最高档结算一次',async()=>{
 const h=host(),game=await load('2048',h);await game.run("start('earn')");await game.run('runtime.save({...runtime.progress,current:{...runtime.progress.current,board:[1024,1024,0,0,0,0,0,0,0,0,0,0,0,0,0,0],highest:1024}})');await game.run("move('left')");
 assert.equal(game.window.document.querySelector('.classic-dialog').dataset.kind,'won');assert.equal(h.balance(),980n);assert.equal(game.run('runtime.progress.current.settled'),false);game.close();
 const other=host(),lost=await load('2048',other);await lost.run("start('earn')");await lost.run('runtime.save({...runtime.progress,current:{...runtime.progress.current,board:[2,2,8,16,8,16,32,64,16,32,64,128,32,64,128,256],highest:256}})');await lost.run("move('left')");
 assert.equal(lost.window.document.querySelector('.classic-dialog').dataset.kind,'lost');assert.equal(other.balance(),990n);const balance=other.balance();await lost.run("move('right')");assert.equal(other.balance(),balance);lost.close();
});
