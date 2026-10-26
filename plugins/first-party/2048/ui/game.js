const rules = window.Twenty48Rules, runtime = new window.ClassicGameRuntime('2048');
const get = id => document.getElementById(id);
let busy = true, loaded = false, touch = null, queued = null;
const keys = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' };
function notice(text, error = false) { get('notice').textContent = text; get('notice').dataset.error = error; }
function render() {
  const current = runtime.progress.current, active = current && !current.settled;
  get('balance').textContent = runtime.balance.toLocaleString('zh-CN');
  get('quota').textContent = `今日赚钱 ${runtime.usedToday}/5 局 · 每款游戏独立计数`;
  get('play').disabled = busy || !loaded || !!active;
  get('earn').disabled = busy || !loaded || !!active || !runtime.walletAllowed || runtime.usedToday >= 5;
  get('game').hidden = !current; if (!current) return;
  get('score').textContent = current.score; get('highest').textContent = current.highest;
  get('reward').textContent = current.mode === 'earn' ? `${rules.reward(current.highest)}币` : '免费局';
  get('end').disabled = busy || !active;
  get('directions').querySelectorAll('button').forEach(button => { button.disabled = busy || !active; });
  const board = get('board');
  if (!board.childElementCount) board.replaceChildren(...Array.from({ length: 16 }, () => { const cell = document.createElement('span'); cell.className = 'tile'; return cell; }));
  current.board.forEach((value, index) => { const cell = board.children[index]; cell.dataset.new = cell.textContent !== String(value); cell.dataset.value = value; cell.dataset.large = value >= 64; cell.textContent = String(value); cell.setAttribute('aria-label', `${Math.floor(index / 4) + 1}行${index % 4 + 1}列：${value || '空'}`); });
}
async function act(operation) {
  if (busy || !loaded) return; busy = true;
  try { await operation(); } catch (error) { notice(window.FirstPartyRuntime.message(error), true); }
  finally { busy = false; render(); const pending = queued; queued = null; if (pending && runtime.progress.current && !runtime.progress.current.settled && !document.querySelector('.classic-dialog[open]')) setTimeout(() => void move(pending), 0); }
}
async function animate(routes) {
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || !get('board').children[0]?.animate) return () => {};
  const board = get('board'), cells = [...board.children].slice(0,16), ghosts = [];
  const bounds = board.getBoundingClientRect(), rectangles = cells.map(cell => cell.getBoundingClientRect());
  board.style.position = 'relative'; board.dataset.moving = 'true';
  const cleanup = () => { ghosts.forEach(ghost => ghost.remove()); board.dataset.moving = 'false'; };
  try {
    const animations = routes.map(route => {
      const source = rectangles[route.from], target = rectangles[route.to], ghost = document.createElement('span');
      ghost.className = 'tile moving-tile'; ghost.dataset.value = route.value; ghost.dataset.large = route.value >= 64; ghost.textContent = route.value;
      Object.assign(ghost.style, {position:'absolute',left:`${source.left-bounds.left-board.clientLeft}px`,top:`${source.top-bounds.top-board.clientTop}px`,width:`${source.width}px`,height:`${source.height}px`,zIndex:'2',pointerEvents:'none'});
      board.append(ghost); ghosts.push(ghost);
      return ghost.animate([{transform:'translate(0,0)'},{transform:`translate(${target.left-source.left}px,${target.top-source.top}px)`}],{duration:180,easing:'cubic-bezier(.22,1,.36,1)',fill:'forwards'}).finished.catch(()=>{});
    });
    await Promise.all(animations);return cleanup;
  } catch(error) {cleanup();throw error;}
}
async function start(mode) {
  await act(async () => {
    const board = rules.create(Math.random);
    await runtime.begin({ board, score: 0, highest: Math.max(...board), moves: 0, status: 'playing' }, mode, 20);
    notice(mode === 'earn' ? '已扣20喵币，按本局最高数字一次结算。' : '免费练习已开始，不扣币、不领奖。');
    get('board').focus();
  });
}
async function move(direction) {
  const current = runtime.progress.current;
  if (!current || current.settled) return;
  if (busy) { queued = direction; return; }
  const next = rules.shift(current.board, direction); if (!next.changed) return;
  await act(async () => {
    const animation = animate(next.routes);
    const board = rules.add(next.board), highest = Math.max(current.highest, ...board), ended = !rules.canMove(board);
    const saving = runtime.update({ ...current, board, highest, score: current.score + next.score, moves: current.moves + 1, status: ended ? (highest >= 2048 ? 'won' : 'finished') : 'playing' }, ended ? rules.reward(highest) : null).then(() => null, error => error);
    const cleanup = await animation; const failure = await saving; if (failure) {cleanup();throw failure;}
    render();cleanup();
    const merged = new Map(); for (const route of next.routes) merged.set(route.to, (merged.get(route.to) ?? 0) + 1);
    [...get('board').children].slice(0,16).forEach((cell,index) => { cell.classList.remove('tile-spawn','tile-merge'); if (board[index] && !next.board[index]) cell.classList.add('tile-spawn'); else if (merged.get(index) > 1) cell.classList.add('tile-merge'); });
    if (ended) window.ClassicGameUI.show(highest >= 2048 ? 'won' : 'lost', highest >= 2048 ? '挑战完成' : '棋盘已经满啦', `本局最高数字${highest}。${current.mode === 'earn' ? `已结算${rules.reward(highest)}喵币。` : '免费游玩不结算喵币。'}`);
    else if (highest >= 2048 && current.highest < 2048) window.ClassicGameUI.show('won', '合成2048！', '已经达成目标，可以继续挑战更高数字，或结束本局结算。本次展示不会提前发放或重复发放奖励。', '继续挑战');
    if (ended) notice(`没有可移动的格子，本局结束。${current.mode === 'earn' ? `已获得${rules.reward(highest)}喵币。` : '免费局不结算喵币。'}`);
    else if (highest >= 2048 && current.highest < 2048) notice('合成2048啦！可以继续挑战，或结束本局领取本档奖励。');
  });
}
get('play').onclick = () => start('play'); get('earn').onclick = () => start('earn');
get('end').onclick = () => act(async () => { const current = runtime.progress.current; if (!current || current.settled) return; const reward = rules.reward(current.highest); await runtime.update({ ...current, status: current.highest >= 2048 ? 'won' : 'finished' }, reward); window.ClassicGameUI.show(current.highest >= 2048 ? 'won' : 'finished', current.highest >= 2048 ? '挑战完成' : '本局已结束', `最高数字${current.highest}。${current.mode === 'earn' ? `已结算${reward}喵币。` : '免费游玩不结算喵币。'}`); notice(`本局已结束。${current.mode === 'earn' ? `按最高数字${current.highest}一次获得${reward}喵币。` : '免费局不结算喵币。'}`); });
document.addEventListener('keydown', event => { if (!document.querySelector('.classic-dialog[open]') && keys[event.key] && !event.target.closest('select,input,textarea') && runtime.progress.current && !runtime.progress.current.settled) { event.preventDefault(); void move(keys[event.key]); } });
get('directions').onclick = event => { const button = event.target.closest('[data-direction]'); if (button) void move(button.dataset.direction); };
get('board').onpointerdown = event => { touch = { x: event.clientX, y: event.clientY }; event.currentTarget.setPointerCapture(event.pointerId); };
get('board').onpointerup = event => { if (!touch) return; const dx = event.clientX - touch.x, dy = event.clientY - touch.y; touch = null; if (Math.max(Math.abs(dx), Math.abs(dy)) < 20) return; void move(Math.abs(dx) > Math.abs(dy) ? dx > 0 ? 'right' : 'left' : dy > 0 ? 'down' : 'up'); };
get('board').onpointercancel = () => { touch = null; };
void runtime.initialize().then(() => { loaded = true; busy = false; render(); notice(runtime.progress.current && !runtime.progress.current.settled ? '已恢复未完成棋局，不会重复扣费。' : '选择免费游玩或赚钱模式，合并相同数字。'); }).catch(error => { notice(window.FirstPartyRuntime.message(error), true); });
