import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const sandbox={window:{},Math};
vm.runInNewContext(readFileSync(new URL('../first-party/my-home/ui/fishing.js',import.meta.url),'utf8'),sandbox);
const fishing=sandbox.window.HomeFishing;

test('大鱼活动区更短，不同抛竿轨迹不同，鱼钩点击上升松手下落',()=>{
  assert.ok(fishing.create(100,1).span>fishing.create(14000,1).span);
  const first=fishing.create(3000,12),second=fishing.create(3000,82);
  for(let i=0;i<120;i++){fishing.step(first,1/60);fishing.step(second,1/60);}
  assert.notEqual(first.target,second.target);
  const game=fishing.create(100,1);fishing.lift(game);fishing.step(game,.08);assert.ok(game.hook>.5);
  const high=game.hook;for(let i=0;i<90;i++)fishing.step(game,1/60);assert.ok(game.hook<high);
  game.hook=.5;game.velocity=-.8;fishing.lift(game);fishing.step(game,.04);assert.ok(game.hook>.5);
});

test('不同帧率相同输入运动一致，位置有界，不操作会脱钩',()=>{
  const a=fishing.create(8000,99),b=fishing.create(8000,99);
  for(let i=0;i<24;i++)fishing.step(a,1/24,true);
  for(let i=0;i<60;i++)fishing.step(b,1/60,true);
  assert.ok(Math.abs(a.hook-b.hook)<.00001);assert.ok(Math.abs(a.target-b.target)<.00001);
  const idle=fishing.create(8000,19);
  for(let i=0;i<1200;i++)fishing.step(idle,1/60);
  assert.equal(idle.status,'escaped');assert.ok(idle.hook>=.025&&idle.hook<=.975);
  const progress=idle.progress;fishing.lift(idle);fishing.step(idle,1/60,true);assert.equal(idle.progress,progress);
});

test('大小鱼都能通过追随活动区攒满进度，终态不会重复推进',()=>{
  for(const grams of [100,4000,14000])for(const seed of [1,17,93]){
    const game=fishing.create(grams,seed);
    for(let i=0;i<3600&&game.status==='playing';i++){
      const desired=Math.max(-.5,Math.min(.5,(game.target-game.hook)*5));
      fishing.step(game,1/120,game.velocity<desired);
    }
    assert.equal(game.status,'caught',`grams=${grams},seed=${seed}`);
    const before=game.hook;fishing.step(game,1/60);assert.equal(game.hook,before);
  }
});
