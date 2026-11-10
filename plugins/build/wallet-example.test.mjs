import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';

const source=readFileSync(new URL('../examples/wallet-challenge/ui/main.js',import.meta.url),'utf8');
const clone=value=>structuredClone(value);
const tick=()=>new Promise(resolve=>setImmediate(resolve));
async function until(check){for(let n=0;n<80;n++){if(check())return;await tick();}assert.fail('示例未完成异步操作');}
const backend=()=>({saved:new Map(),ledger:new Map(),balance:10,calls:[],round:0});
async function mount(data=backend(),options={}){
 const ids=['balance','progress','start','answer-form','question','answer','submit','status','retry','close'];
 const elements=new Map(ids.map(id=>[id,{disabled:true,hidden:false,value:'',textContent:'',handlers:{},addEventListener(type,fn){this.handlers[type]=fn;}}]));
 const runId=options.runId??'1';let closed=false;
 const api={game:{ready:async()=>{},getContext:async()=>({gameRunId:runId,walletPermissionGranted:!options.denied,permissions:[{permission:'wallet.modify',scope:'',decision:options.denied?'denied':'allowed'}]}),random:async()=>.2,close:async()=>{closed=true;}},storage:{
  get:async key=>clone(data.saved.get(key)??null),
  set:async(key,value)=>{
   if(options.failPendingWrite&&value.pending){options.failPendingWrite=false;throw new Error('pending-write-failed');}
   if(options.failFinalWrite&&!value.pending){options.failFinalWrite=false;throw new Error('final-write-failed');}
   data.saved.set(key,clone(value));
   if(options.losePendingResponse&&value.pending){options.losePendingResponse=false;throw new Error('pending-response-lost');}
  }
 },wallet:{getBalance:async()=>({coins:String(data.balance)}),changeCoins:async request=>{
  assert.deepEqual(clone(data.saved.get('state.main').pending.request),clone(request),'钱包之前必须持久化完整请求');data.calls.push(clone(request));
  const old=data.ledger.get(request.request_id);
  if(old){assert.deepEqual(old.request,clone(request));return {...old.result,duplicate:true,balance:String(data.balance)};}
  if(request.game_run_id!==runId)throw new Error('old-run-not-submitted');
  if(options.denied)throw new Error('permission-denied');
  if(data.balance+request.delta<0)throw new Error('insufficient-coins');
  data.balance+=request.delta;const result={balance:String(data.balance),duplicate:false};data.ledger.set(request.request_id,{request:clone(request),result});
  if(options.loseWalletResponse){options.loseWalletResponse=false;throw new Error('wallet-response-lost');}
  return result;
 }}};
 vm.runInContext(source,vm.createContext({window:{yanmew:api},document:{querySelector:selector=>elements.get(selector.slice(1))},crypto:{randomUUID:()=>`round-${++data.round}`},structuredClone,console}));
 await until(()=>!elements.get('close').disabled);
 const fire=async(id,type='click')=>{elements.get(id).handlers[type]({preventDefault(){}});await tick();await until(()=>!elements.get('close').disabled);};
 return {data,elements,options,fire,closed:()=>closed,saved:()=>data.saved.get('state.main')};
}
test('开始扣一次、答错不扣费、答对奖一次，连续多局保持进度',async()=>{
 const game=await mount();await game.fire('start');assert.equal(game.data.balance,9);assert.equal(game.saved().progress.rounds,1);
 game.elements.get('answer').value='0';await game.fire('answer-form','submit');assert.equal(game.data.calls.length,1);
 game.elements.get('answer').value='4';await game.fire('answer-form','submit');assert.equal(game.data.balance,11);assert.equal(game.saved().progress.wins,1);assert.equal(game.saved().progress.round,null);
 await game.fire('start');assert.equal(game.data.balance,10);assert.equal(game.saved().progress.rounds,2);
});
test('快速重复点击不产生第二笔扣费或奖励',async()=>{
 const game=await mount();const click=game.elements.get('start').handlers.click;click();click();await until(()=>!game.elements.get('close').disabled);assert.equal(game.data.ledger.size,1);
 game.elements.get('answer').value='4';const submit=game.elements.get('answer-form').handlers.submit;submit({preventDefault(){}});submit({preventDefault(){}});await until(()=>!game.elements.get('close').disabled);assert.equal(game.data.ledger.size,2);assert.equal(game.saved().progress.wins,1);
});
test('拒绝权限不调用钱包，不能开始',async()=>{
 const game=await mount(backend(),{denied:true});assert.equal(game.elements.get('start').disabled,true);await game.fire('start');assert.equal(game.data.calls.length,0);assert.equal(game.data.balance,10);
});
test('待请求写失败不扣款，写入响应丢失重读后只核对原请求',async()=>{
 const game=await mount(backend(),{failPendingWrite:true});await game.fire('start');assert.equal(game.data.calls.length,0);assert.equal(game.data.balance,10);
 game.options.losePendingResponse=true;await game.fire('start');assert.ok(game.saved().pending);assert.equal(game.data.calls.length,0);
 const pending=clone(game.saved().pending.request);await game.fire('retry');assert.deepEqual(game.data.calls[0],pending);assert.equal(game.data.balance,9);
});
test('扣款响应未知，关闭重开使用原运行及请求ID，不重复扣款',async()=>{
 const game=await mount(backend(),{loseWalletResponse:true});await game.fire('start');assert.equal(game.data.balance,9);const request=clone(game.saved().pending.request);await game.fire('close');assert.equal(game.closed(),true);
 const reopened=await mount(game.data,{runId:'2'});assert.deepEqual(reopened.data.calls[1],request);assert.equal(reopened.data.balance,9);assert.equal(reopened.saved().pending,null);assert.equal(reopened.saved().progress.rounds,1);
});
test('奖励后进度写失败，重试duplicate只落实一次胜场',async()=>{
 const game=await mount();await game.fire('start');game.options.failFinalWrite=true;game.elements.get('answer').value='4';await game.fire('answer-form','submit');assert.equal(game.data.balance,11);const request=clone(game.saved().pending.request);
 await game.fire('retry');assert.deepEqual(game.data.calls.at(-1),request);assert.equal(game.data.balance,11);assert.equal(game.saved().progress.wins,1);assert.equal(game.data.ledger.size,2);
});
test('旧运行未提交请求保留pending并阻止新结算，不清档退款',async()=>{
 const game=await mount(backend(),{losePendingResponse:true});await game.fire('start');const pending=clone(game.saved().pending);const reopened=await mount(game.data,{runId:'2'});
 assert.deepEqual(reopened.saved().pending,pending);assert.equal(reopened.data.balance,10);assert.equal(reopened.elements.get('start').disabled,true);assert.match(reopened.elements.get('status').textContent,/待结算请求已保留/);
});
test('不兼容存档不覆盖、不调用钱包',async()=>{
 const data=backend();data.saved.set('state.main',{version:99});const game=await mount(data);assert.deepEqual(data.saved.get('state.main'),{version:99});assert.equal(data.calls.length,0);assert.equal(game.elements.get('start').disabled,true);
});
