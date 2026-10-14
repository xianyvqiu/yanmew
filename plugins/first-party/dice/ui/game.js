
// 模式和日期次数属于插件；余额只读取全局钱包。
const runtime = new FirstPartyRuntime({ day: FirstPartyRuntime.day(), usedToday: 0, walletMode: true });
let ready = false;
function syncQuota() {
  const p = runtime.progress;
  state.usedToday = p.day === FirstPartyRuntime.day() ? p.usedToday : 0;
}
function renderMode() {
  permBtn.textContent = state.walletEnabled ? '喵币：开' : '喵币：关';
  permBtn.classList.toggle('off', !state.walletEnabled);
  playmodeEl.classList.toggle('show', !state.walletEnabled);
  renderBalance();
}
async function refreshWallet() {
  if (!ready || state.busy) return;
  try {
    await runtime.refresh();
    if (state.busy) return;
    state.balance = runtime.balance;
    if (!runtime.walletAllowed) state.walletEnabled = false;
    syncQuota(); renderMode(); renderQuota(); refreshUI();
  } catch (error) { footHint.textContent = FirstPartyRuntime.message(error); }
}
window.addEventListener('focus', refreshWallet);
setInterval(refreshWallet, 60000);
/* ============================================================
   常量
   ============================================================ */
const BET = 3;
const LUCKY_BONUS = 2;
const MIN_BET = 3;
const DAILY_LIMIT = 20;

/* 骰子点位：3×3 网格中的坐标 [row, col] */
const DOT_POSITIONS = {
  1: [[2, 2]],
  2: [[1, 1], [3, 3]],
  3: [[1, 1], [2, 2], [3, 3]],
  4: [[1, 1], [1, 3], [3, 1], [3, 3]],
  5: [[1, 1], [1, 3], [2, 2], [3, 1], [3, 3]],
  6: [[1, 1], [1, 3], [2, 1], [2, 3], [3, 1], [3, 3]],
};

const state = {
  balance: 0n,
  usedToday: 0,
  walletEnabled: false,
  busy: true,
};

/* ============================================================
   DOM
   ============================================================ */
const $ = s => document.querySelector(s);

const balanceEl   = $('#balance');
const quotaUsedEl = $('#quotaUsed');
const quotaTotalEl= $('#quotaTotal');
const permBtn     = $('#permBtn');
const playmodeEl  = $('#playmode');

const playerDieEl = $('#playerDie');
const aiDieEl     = $('#aiDie');

const resultArea  = $('#resultArea');
const stampEl     = $('#stamp');
const stampTextEl = $('#stampText');
const stampDeltaEl= $('#stampDelta');
const luckyTagEl  = $('#luckyTag');

const rollBtn     = $('#rollBtn');
const rollBtnText = $('#rollBtnText');
const rollBtnSub  = $('#rollBtnSub');
const footHint    = $('#footHint');

const sleep = ms => new Promise(r => setTimeout(r, ms));
const randInt = (min, max) => Math.floor(Math.random() * (max - min + 1)) + min;

/* ============================================================
   骰子渲染
   ============================================================ */
function renderDie(el, value) {
  el.innerHTML = '';
  DOT_POSITIONS[value].forEach(([r, c]) => {
    const dot = document.createElement('span');
    dot.className = 'dot';
    dot.style.gridRow = r;
    dot.style.gridColumn = c;
    el.appendChild(dot);
  });
}

/* ============================================================
   界面刷新
   ============================================================ */
function renderBalance() {
  balanceEl.textContent = state.walletEnabled ? state.balance : '—';
}
function renderQuota() {
  syncQuota();
  quotaUsedEl.textContent = state.usedToday;
}
function canPlay() {
  syncQuota();
  if (!ready || runtime.blocked || state.busy) return false;
  if (state.busy) return false;
  if (state.usedToday >= DAILY_LIMIT) return false;
  if (state.walletEnabled && state.balance < MIN_BET) return false;
  return true;
}
function refreshUI() {
  const ok = canPlay();
  rollBtn.disabled = !ok;

  if (state.usedToday >= DAILY_LIMIT) {
    footHint.textContent = '今天玩够啦，明天再来';
  } else if (state.walletEnabled && state.balance < MIN_BET) {
    footHint.textContent = `至少需要 ${MIN_BET} 喵币才能下注`;
  } else {
    footHint.textContent = '点击开始';
  }

  if (state.busy) {
    rollBtnText.textContent = '掷骰中…';
    rollBtnSub.textContent = '';
  } else if (state.walletEnabled) {
    rollBtnText.textContent = '掷骰子';
    rollBtnSub.textContent = `赌注 ${BET}`;
  } else {
    rollBtnText.textContent = '掷骰子';
    rollBtnSub.textContent = '纯玩';
  }
}

/* ============================================================
   权限
   ============================================================ */
permBtn.addEventListener('click', async () => {
  if (!ready || state.busy || runtime.blocked) return;
  state.busy = true; permBtn.disabled = true;
  let errorText = '';
  try {
    await runtime.refresh(); state.balance = runtime.balance;
    if (!state.walletEnabled && !runtime.walletAllowed) throw new Error('请到插件管理允许钱包权限，并重新打开游戏。');
    const enabled = !state.walletEnabled;
    await runtime.save({ ...runtime.progress, walletMode: enabled });
    state.walletEnabled = enabled;
    renderMode(); refreshUI();
  } catch (error) { errorText = FirstPartyRuntime.message(error); }
  finally { state.busy = false; permBtn.disabled = false; refreshUI(); if (errorText) footHint.textContent = errorText; }
});

/* ============================================================
   判定
   ============================================================ */
function judge(player, ai) {
  if (player === ai) return 'draw';
  return player > ai ? 'win' : 'lose';
}

/* ============================================================
   印章
   ============================================================ */
function showStamp(result) {
  stampEl.classList.remove('show', 'win', 'lose', 'draw');
  void stampEl.offsetWidth;

  stampTextEl.textContent = { win: '你赢了', lose: '你输了', draw: '平局' }[result];

  let deltaText = '纯玩';
  if (state.walletEnabled) {
    const d = { win: BET, lose: -BET, draw: 0 }[result];
    deltaText = d > 0 ? `+${d}` : d < 0 ? `${d}` : '±0';
  }
  stampDeltaEl.textContent = deltaText;

  stampEl.classList.add(result, 'show');
}

/* ============================================================
   金币获得动画
   ============================================================ */
async function playGainAnimation(fromEl, toEl, count, lucky) {
  const fromRect = fromEl.getBoundingClientRect();
  const toRect   = toEl.getBoundingClientRect();
  const fx = fromRect.left + fromRect.width / 2;
  const fy = fromRect.top  + fromRect.height / 2;
  const tx = toRect.left   + toRect.width / 2;
  const ty = toRect.top    + toRect.height / 2;

  const promises = [];

  for (let i = 0; i < count; i++) {
    promises.push(new Promise(resolve => {
      setTimeout(() => {
        const coin = document.createElement('div');
        coin.className = 'coin-sprite' + (lucky ? ' lucky' : '');
        coin.style.left = fx + 'px';
        coin.style.top  = fy + 'px';
        document.body.appendChild(coin);

        const dx = tx - fx;
        const dy = ty - fy;
        const arcX = dx * 0.5 + (Math.random() - 0.5) * 40;
        const arcY = dy * 0.5 - 70 - Math.random() * 30;

        const anim = coin.animate([
          { transform: 'translate(-50%, -50%) scale(1) rotate(0deg)', opacity: 1 },
          { transform: `translate(calc(-50% + ${arcX}px), calc(-50% + ${arcY}px)) scale(1.3) rotate(180deg)`, opacity: 1, offset: 0.55 },
          { transform: `translate(calc(-50% + ${dx}px), calc(-50% + ${dy}px)) scale(0.25) rotate(360deg)`, opacity: 0 }
        ], {
          duration: 700,
          easing: 'cubic-bezier(.4, 0, .2, 1)',
          fill: 'forwards'
        });

        anim.onfinish = () => { coin.remove(); resolve(); };
      }, i * 70);
    }));
  }

  setTimeout(() => {
    toEl.animate([
      { transform: 'scale(1)' },
      { transform: 'scale(1.22)' },
      { transform: 'scale(1)' }
    ], { duration: 380, easing: 'cubic-bezier(.34, 1.56, .64, 1)' });
  }, count * 70 + 500);

  await Promise.all(promises);
}

/* ============================================================
   金币失去动画
   ============================================================ */
async function playLoseAnimation(fromEl, count) {
  const rect = fromEl.getBoundingClientRect();
  const fx = rect.left + rect.width / 2;
  const fy = rect.top  + rect.height / 2;

  fromEl.animate([
    { transform: 'translateX(0) rotate(0)' },
    { transform: 'translateX(-5px) rotate(-2deg)' },
    { transform: 'translateX(5px) rotate(2deg)' },
    { transform: 'translateX(-3px) rotate(-1deg)' },
    { transform: 'translateX(0) rotate(0)' }
  ], { duration: 400, easing: 'cubic-bezier(.4, 0, .2, 1)' });

  const promises = [];

  for (let i = 0; i < count; i++) {
    promises.push(new Promise(resolve => {
      setTimeout(() => {
        const coin = document.createElement('div');
        coin.className = 'coin-sprite lose';
        coin.style.left = fx + 'px';
        coin.style.top  = fy + 'px';
        document.body.appendChild(coin);

        const angle = Math.PI * (0.3 + Math.random() * 0.4);
        const speed = 55 + Math.random() * 35;
        const dx = Math.cos(angle) * speed;
        const dy = -Math.sin(angle) * speed * 0.7;

        const anim = coin.animate([
          { transform: 'translate(-50%, -50%) scale(1) rotate(0deg)', opacity: 1, offset: 0 },
          { transform: `translate(calc(-50% + ${dx}px), calc(-50% + ${dy}px)) scale(1) rotate(140deg)`, opacity: .95, offset: 0.35 },
          { transform: `translate(calc(-50% + ${dx * 1.4}px), calc(-50% + ${dy + 90}px)) scale(0.35) rotate(340deg)`, opacity: 0 }
        ], {
          duration: 780,
          easing: 'cubic-bezier(.4, 0, .6, 1)',
          fill: 'forwards'
        });

        anim.onfinish = () => { coin.remove(); resolve(); };
      }, i * 80);
    }));
  }

  await Promise.all(promises);
}

/* ============================================================
   飘字
   ============================================================ */
function spawnFloatText(text, x, y, type) {
  const el = document.createElement('div');
  el.className = 'float-text ' + type;
  el.textContent = text;
  el.style.left = x + 'px';
  el.style.top  = y + 'px';
  document.body.appendChild(el);

  el.animate([
    { transform: 'translate(-50%, -50%) scale(.7)', opacity: 0 },
    { transform: 'translate(-50%, -90%) scale(1.15)', opacity: 1, offset: .25 },
    { transform: 'translate(-50%, -200%) scale(1)', opacity: 0 }
  ], {
    duration: 1100,
    easing: 'cubic-bezier(.22, 1, .36, 1)',
    fill: 'forwards'
  }).onfinish = () => el.remove();
}

/* ============================================================
   基础结算
   ============================================================ */
async function applyBaseReward(result) {
  if (!state.walletEnabled) return;

  const d = { win: BET, lose: -BET, draw: 0 }[result];
  if (d === 0) return;

  // 基础奖励已连同幸运奖励一次提交，保留两段展示。

  balanceEl.textContent = state.balance;
  balanceEl.classList.remove('up', 'down');
  void balanceEl.offsetWidth;
  balanceEl.classList.add(d > 0 ? 'up' : 'down');
  setTimeout(() => balanceEl.classList.remove('up', 'down'), 900);

  const resultRect = resultArea.getBoundingClientRect();
  const walletRect = balanceEl.getBoundingClientRect();

  if (d > 0) {
    spawnFloatText(`+${d}`,
      resultRect.left + resultRect.width / 2,
      resultRect.top  + resultRect.height / 2 - 20,
      'gain');
    await playGainAnimation(resultArea, balanceEl, d, false);
  } else {
    spawnFloatText(`${d}`,
      walletRect.left + walletRect.width / 2,
      walletRect.top  + walletRect.height / 2 + 20,
      'lose');
    await playLoseAnimation(balanceEl, Math.abs(d));
  }
}

/* ============================================================
   幸运 6 结算
   ============================================================ */
async function applyLuckyBonus() {
  // 贴纸
  luckyTagEl.classList.remove('show');
  void luckyTagEl.offsetWidth;
  luckyTagEl.classList.add('show');

  // 骰子金色光晕
  playerDieEl.classList.add('lucky');

  // 飘字
  const dieRect = playerDieEl.getBoundingClientRect();
  spawnFloatText(`+${LUCKY_BONUS}`,
    dieRect.left + dieRect.width / 2,
    dieRect.top  - 8,
    'lucky');

  if (!state.walletEnabled) {
    await sleep(900);
    return;
  }

  state.balance += BigInt(LUCKY_BONUS);
  balanceEl.textContent = state.balance;
  balanceEl.classList.remove('up', 'down');
  void balanceEl.offsetWidth;
  balanceEl.classList.add('up');
  setTimeout(() => balanceEl.classList.remove('up'), 900);

  await playGainAnimation(playerDieEl, balanceEl, LUCKY_BONUS, true);
}

/* ============================================================
   滚动控制
   ============================================================ */
function startRolling(el) {
  el.classList.remove('lucky', 'pop');
  el.classList.add('rolling');
  el._rollTimer = setInterval(() => {
    renderDie(el, randInt(1, 6));
  }, 70);
}

function stopRolling(el, value) {
  if (el._rollTimer) {
    clearInterval(el._rollTimer);
    el._rollTimer = null;
  }
  renderDie(el, value);
  el.classList.remove('rolling');
  el.classList.remove('pop');
  void el.offsetWidth;
  el.classList.add('pop');
}

/* ============================================================
   清场
   ============================================================ */
function clearBoard() {
  stampEl.classList.remove('show', 'win', 'lose', 'draw');
  stampTextEl.textContent = '——';
  stampDeltaEl.textContent = '';
  luckyTagEl.classList.remove('show');
  playerDieEl.classList.remove('lucky');
  aiDieEl.classList.remove('lucky');
}

/* ============================================================
   主流程
   ============================================================ */
async function animateRoll(playerValue, aiValue) {

  state.busy = true;
  refreshUI();



  clearBoard();

  // 双方同时滚动
  startRolling(playerDieEl);
  startRolling(aiDieEl);

  await sleep(720);

  // AI 先停，制造悬念
  stopRolling(aiDieEl, aiValue);
  await sleep(360);

  // 玩家后停
  stopRolling(playerDieEl, playerValue);
  await sleep(320);

  // 判定
  const result = judge(playerValue, aiValue);
  showStamp(result);

  await sleep(360);

  // 基础结算
  await applyBaseReward(result);

  // 幸运 6
  if (playerValue === 6) {
    await sleep(180);
    await applyLuckyBonus();
  }

  // 配额


  await sleep(500);

  state.busy = false;
  refreshUI();
}

/* ============================================================
   绑定
   ============================================================ */
rollBtn.addEventListener('click', roll);

window.addEventListener('keydown', e => {
  if (e.repeat) return;
  if (e.code === 'Space' || e.code === 'Enter') {
    if (!canPlay()) return;
    e.preventDefault();
    roll();
  }
});

/* ============================================================
   初始化
   ============================================================ */
(async function init() {
  permBtn.disabled = true;
  try {
    await runtime.initialize(); state.balance = runtime.balance;
    state.walletEnabled = runtime.progress.walletMode !== false && runtime.walletAllowed;
    ready = true; state.busy = false;
    renderMode();

  quotaTotalEl.textContent = `/ ${DAILY_LIMIT}`;

  // 初始骰子
  renderDie(playerDieEl, 1);
  renderDie(aiDieEl, 1);

  renderBalance();
  renderQuota();
  refreshUI();

  } catch (error) { footHint.textContent = FirstPartyRuntime.message(error); }
  finally { permBtn.disabled = !ready; }
})();

async function beginRound() {
  await runtime.refresh(); state.balance = runtime.balance;
  if (state.walletEnabled && !runtime.walletAllowed) throw new Error('钱包权限已撤销，请重新打开游戏。');
  syncQuota();
  if (state.usedToday >= DAILY_LIMIT) throw new Error('今日次数已用完');
  const required = MIN_BET;
  if (state.walletEnabled && runtime.balance < BigInt(required)) throw new Error(`喵币不足（需 ${required}）`);
  const roundId = crypto.randomUUID();
  await runtime.save({ ...runtime.progress, day: FirstPartyRuntime.day(), usedToday: state.usedToday + 1 });
  renderQuota();
  return roundId;
}

async function roll() {
  if (!canPlay()) return;
  state.busy = true; permBtn.disabled = true; refreshUI();
  let errorText = '';
  try {
    const roundId = await beginRound();
    const playerValue = 1 + Math.floor(await runtime.sdk.game.random() * 6);
    const aiValue = 1 + Math.floor(await runtime.sdk.game.random() * 6);
    const result = judge(playerValue, aiValue);
    const bonus = playerValue === 6 ? LUCKY_BONUS : 0;
    const delta = {win: BET, lose: -BET, draw: 0}[result] + bonus;
    const after = { ...runtime.progress, lastRound: { roundId, playerValue, aiValue, result, bonus, walletMode: state.walletEnabled } };
    if (state.walletEnabled) await runtime.commit(after, delta, '掷骰子：' + ({win:'胜利',lose:'失败',draw:'平局'}[result]) + (bonus ? '，幸运6' : ''), after.lastRound, true);
    else await runtime.save(after);
    state.balance = runtime.balance - (state.walletEnabled ? BigInt(bonus) : 0n);
    await animateRoll(playerValue, aiValue);
    state.balance = runtime.balance; renderBalance();
  } catch (error) { errorText = FirstPartyRuntime.message(error); }
  finally {
    state.busy = false; permBtn.disabled = false; renderQuota(); refreshUI();
    if (errorText) footHint.textContent = errorText;
  }
}
