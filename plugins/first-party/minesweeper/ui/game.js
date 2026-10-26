const rules = window.MinesweeperRules, runtime = new window.ClassicGameRuntime('扫雷');
const get = id => document.getElementById(id);
let busy = true, loaded = false;
const icons = {
  flag: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M5 21V3h13l-3 5 3 5H5M2 21h8"/></svg>',
  mine: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><circle cx="12" cy="12" r="5"/><path d="M12 1v5m0 12v5M1 12h5m12 0h5M4 4l4 4m8 8 4 4M4 20l4-4m8-8 4-4"/></svg>',
};
function notice(text, error = false) { get('notice').textContent = text; get('notice').dataset.error = error; }
function render() {
  const current = runtime.progress.current, active = current && !current.settled;
  get('balance').textContent = runtime.balance.toLocaleString('zh-CN');
  get('quota').textContent = `今日赚钱 ${runtime.usedToday}/5 局 · 每款游戏独立计数`;
  for (const id of ['play', 'earn', 'level']) get(id).disabled = busy || !loaded || !!active || (id === 'earn' && (!runtime.walletAllowed || runtime.usedToday >= 5));
  const level = rules.levels[get('level').value]; get('earn-label').textContent = `开始赚钱局 · ${level.cost}喵币`;
  get('game').hidden = !current; if (!current) return;
  get('end').disabled = busy || current.settled;
  get('remaining').textContent = current.mines - current.cells.filter(cell => cell.mark === 1).length;
  get('mode').textContent = current.mode === 'earn' ? `赚钱 · ${current.cost}币` : '免费游玩';
  const board = get('board'); board.style.setProperty('--columns', current.columns);
  if (board.childElementCount !== current.cells.length) {
    board.replaceChildren(...current.cells.map((_, index) => { const button = document.createElement('button'); button.type = 'button'; button.dataset.index = index; return button; }));
  }
  current.cells.forEach((cell, index) => {
    const button = board.children[index], showMine = cell.mine && current.status === 'lost';
    button.disabled = busy || current.settled;
    button.dataset.open = cell.open; button.dataset.number = cell.number; button.dataset.mine = showMine;
    button.innerHTML = showMine ? icons.mine : cell.mark === 1 ? icons.flag : cell.open ? (cell.number ? String(cell.number) : '') : cell.mark === 2 ? '?' : '';
    button.setAttribute('aria-label', `${Math.floor(index / current.columns) + 1}行${index % current.columns + 1}列，${showMine ? '地雷' : cell.mark === 1 ? '旗子' : cell.open ? cell.number + '个邻雷' : cell.mark === 2 ? '问号' : '未翻开'}`);
  });
}
async function act(operation) {
  if (busy || !loaded) return; busy = true;
  try { await operation(); }
  catch (error) { notice(window.FirstPartyRuntime.message(error), true); }
  finally { busy = false; render(); }
}
async function start(mode) {
  await act(async () => {
    const board = rules.create(get('level').value); await runtime.begin(board, mode, board.cost);
    notice(mode === 'earn' ? `已扣${board.cost}喵币。通关奖励${board.cost * 2}喵币，放弃或踩雷不返还入场费。` : '免费模式已开始，尽情练习吧。');
  });
}
async function move(action, index) {
  const current = runtime.progress.current; if (!current || current.settled || busy) return;
  const board = action === 'mark' ? rules.mark(current, index) : action === 'chord' ? rules.chord(current, index) : rules.open(current, index);
  if (JSON.stringify(board) === JSON.stringify(current)) return;
  await act(async () => {
    const done = ['won', 'lost'].includes(board.status), reward = board.status === 'won' ? board.cost * 2 : 0;
    await runtime.update(board, done ? reward : null);
    if (done) notice(board.status === 'won' ? `全部排除，通关成功！${current.mode === 'earn' ? `已获得${reward}喵币。` : '免费局不结算喵币。'}` : '踩到地雷啦，本局结束。可以重新选择难度或模式。');
    if (done) window.ClassicGameUI.show(board.status, board.status === 'won' ? '通关成功' : '这次碰到地雷啦', board.status === 'won' ? `你找出了全部安全格。${current.mode === 'earn' ? `已获得${reward}喵币。` : '免费游玩不结算喵币。'}` : '本局已经结束。数字表示周围八格中的地雷数量，下一局可以结合标旗再判断。');
  });
}
get('play').onclick = () => start('play'); get('earn').onclick = () => start('earn'); get('level').onchange = render;
get('board').onclick = event => { const cell = event.target.closest('[data-index]'); if (cell) void move('open', Number(cell.dataset.index)); };
get('board').ondblclick = event => { const cell = event.target.closest('[data-index]'); if (cell) void move('chord', Number(cell.dataset.index)); };
get('board').oncontextmenu = event => { event.preventDefault(); const cell = event.target.closest('[data-index]'); if (cell) void move('mark', Number(cell.dataset.index)); };
get('board').onkeydown = event => { if (event.key === 'F10' && event.shiftKey) { event.preventDefault(); const cell = event.target.closest('[data-index]'); if (cell) void move('mark', Number(cell.dataset.index)); } };
get('end').onclick = () => act(async () => { await runtime.update({ ...runtime.progress.current, status: 'lost' }, 0); notice('本局已结束，已使用的赚钱次数及入场费不返还。'); });
setInterval(() => {
  const current = runtime.progress.current; if (!current) return;
  const seconds = Math.max(0, Math.floor(((current.endedAt ?? Date.now()) - current.startedAt) / 1000));
  get('elapsed').textContent = `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}, 1000);
void runtime.initialize().then(() => { loaded = true; busy = false; render(); notice(runtime.progress.current && !runtime.progress.current.settled ? '已恢复未完成的棋局，不会重复收取入场费。' : '选好难度，再选择免费游玩或赚钱模式。'); }).catch(error => { notice(window.FirstPartyRuntime.message(error), true); });

get('help').onclick = () => window.ClassicGameUI.show('help', '扫雷游玩帮助', '目标：翻开所有不含地雷的格子即可通关，不要求把每颗雷都标旗。\n\n数字：表示这个格子周围八格中有几颗雷。空白区域会自动展开。\n\n左键点击翻格；右键依次切换旗子、问号和取消。旗子用于标记你认为有雷的位置。\n\n双击已翻开的数字：周围旗数与数字一致时，展开其他邻格。旗子标错仍可能踩雷，请先判断。\n\n键盘：Tab选择格子，Enter或空格翻开，Shift+F10标旗。首击安全。\n\n免费游玩不限次数；赚钱模式初级5币、中级20币、高级50币，成功奖励分别10、40、100币。每日5局，放弃或失败不退入场费。', '开始排雷');
