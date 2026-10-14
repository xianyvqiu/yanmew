
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
  if (!ready || (state.playing || state.busy)) return;
  try {
    await runtime.refresh();
    if ((state.playing || state.busy)) return;
    state.balance = runtime.balance;
    if (!runtime.walletAllowed) state.walletEnabled = false;
    syncQuota(); renderMode(); renderQuota(); refreshOverlay();
  } catch (error) { footHint.textContent = FirstPartyRuntime.message(error); }
}
window.addEventListener('focus', refreshWallet);
setInterval(refreshWallet, 60000);
/* ============================================================
   常量
   ============================================================ */
const COST = 8;                 // 开一局消耗
const DURATION = 20;            // 秒
const DAILY_LIMIT = 20;

/* ============================================================
   状态
   ============================================================ */
const state = {
  balance: 0n,
  usedToday: 0,
  walletEnabled: false,
  playing: false,
  busy: true,
};

/* ============================================================
   DOM
   ============================================================ */
const $ = s => document.querySelector(s);
const balanceEl    = $('#balance');
const quotaUsedEl  = $('#quotaUsed');
const quotaTotalEl = $('#quotaTotal');
const permBtn      = $('#permBtn');
const playmodeEl   = $('#playmode');
const skyEl        = $('#sky');
const basketEl     = $('#basket');
const timeEl       = $('#timeEl');
const scoreEl      = $('#scoreEl');
const earnEl       = $('#earnEl');
const overlay      = $('#overlay');
const overlayIcon  = $('#overlayIcon');
const overlayTitle = $('#overlayTitle');
const overlayDesc  = $('#overlayDesc');
const overlayCost  = $('#overlayCost');
const overlayScore = $('#overlayScore');
const overlayReward= $('#overlayReward');
const startBtn     = $('#startBtn');
const footHint     = $('#footHint');

const sleep = ms => new Promise(r => setTimeout(r, ms));

/* ============================================================
   游戏状态
   ============================================================ */
let stars = [];
let raf = null;
let timeLeft = 0;
let score = 0;
let lastTs = 0;
let spawnTimer = 0;
let basketX = 0.5;

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

function canStart() {
  syncQuota();
  if (!ready || runtime.blocked || state.busy) return false;
  if (state.playing) return false;
  if (state.usedToday >= DAILY_LIMIT) return false;
  if (state.walletEnabled && state.balance < COST) return false;
  return true;
}

function getBlockReason() {
  syncQuota();
  if (state.playing) return null;
  if (state.usedToday >= DAILY_LIMIT) return '今日次数已用完';
  if (state.walletEnabled && state.balance < COST) return `喵币不足（需 ${COST}）`;
  return null;
}

function refreshOverlay() {
  if (state.playing) return;
  if (!ready || state.busy || runtime.blocked) { startBtn.disabled = true; return; }

  const reason = getBlockReason();
  if (reason) {
    startBtn.disabled = true;
    startBtn.textContent = reason;
  } else {
    startBtn.disabled = false;
    startBtn.textContent = overlayTitle.textContent === '本局结束' ? '再来一局' : '开始 20 秒';
  }

  if (state.usedToday >= DAILY_LIMIT) {
    footHint.textContent = '今天玩够啦，明天再来';
  } else if (state.walletEnabled && state.balance < COST) {
    footHint.textContent = `至少需要 ${COST} 喵币才能开一局`;
  } else {
    footHint.textContent = '点击开始';
  }
}

function showStartPanel() {
  overlayIcon.textContent = '⭐';
  overlayTitle.textContent = '接星星';
  overlayDesc.style.display = '';
  overlayDesc.textContent = '移动鼠标或方向键，接住掉落的星星';
  overlayCost.style.display = '';
  overlayCost.textContent = `开一局消耗 ${COST} 喵币`;
  overlayScore.style.display = 'none';
  overlayReward.style.display = 'none';
  overlay.classList.remove('hide');
  refreshOverlay();
}

function showResultPanel() {
  /* 结算：每颗 +1，成本 COST */
  const reward = score;             // 接几颗 = 加几喵币
  const delta = reward - COST;      // 净收益

  /* 按得分选择图标 */
  if (score >= 20)      overlayIcon.textContent = '🏆';
  else if (score >= 15) overlayIcon.textContent = '✨';
  else if (score >= COST) overlayIcon.textContent = '⭐';
  else                  overlayIcon.textContent = '💧';

  overlayTitle.textContent = '本局结束';
  overlayDesc.style.display = 'none';
  overlayCost.style.display = 'none';

  overlayScore.style.display = '';
  overlayScore.textContent = `接住 ${score} 颗 · 得到 ${reward} 喵币`;

  overlayReward.style.display = '';
  if (!state.walletEnabled) {
    overlayReward.textContent = '纯玩 · 不修改喵币';
    overlayReward.className = 'reward-text draw';
  } else {
    overlayReward.textContent = delta > 0 ? `净赚 +${delta} 喵币` : delta < 0 ? `净亏 ${delta} 喵币` : '保本 ±0';
    overlayReward.className = 'reward-text ' + (delta > 0 ? 'win' : delta < 0 ? 'lose' : 'draw');
  }

  overlay.classList.remove('hide');
  refreshOverlay();
}

/* ============================================================
   权限
   ============================================================ */
permBtn.addEventListener('click', async () => {
  if (!ready || (state.playing || state.busy) || runtime.blocked) return;
  state.busy = true; permBtn.disabled = true;
  let errorText = '';
  try {
    await runtime.refresh(); state.balance = runtime.balance;
    if (!state.walletEnabled && !runtime.walletAllowed) throw new Error('请到插件管理允许钱包权限，并重新打开游戏。');
    const enabled = !state.walletEnabled;
    await runtime.save({ ...runtime.progress, walletMode: enabled });
    state.walletEnabled = enabled;
    renderMode(); refreshOverlay();
  } catch (error) { errorText = FirstPartyRuntime.message(error); }
  finally { state.busy = false; permBtn.disabled = false; refreshOverlay(); if (errorText) footHint.textContent = errorText; }
});

/* ============================================================
   篮子控制
   ============================================================ */
function setBasket(clientX) {
  const r = skyEl.getBoundingClientRect();
  let ratio = (clientX - r.left) / r.width;
  ratio = Math.max(0.06, Math.min(0.94, ratio));
  basketX = ratio;
  basketEl.style.left = (ratio * 100) + '%';
}

skyEl.addEventListener('mousemove', e => {
  if (state.playing) setBasket(e.clientX);
});
skyEl.addEventListener('touchmove', e => {
  if (!state.playing) return;
  if (e.touches[0]) setBasket(e.touches[0].clientX);
}, { passive: true });
window.addEventListener('keydown', e => {
  if (!state.playing) return;
  if (e.key === 'ArrowLeft')  { basketX = Math.max(0.06, basketX - 0.05); basketEl.style.left = (basketX * 100) + '%'; }
  if (e.key === 'ArrowRight') { basketX = Math.min(0.94, basketX + 0.05); basketEl.style.left = (basketX * 100) + '%'; }
});

/* ============================================================
   星星
   ============================================================ */
function spawnStar() {
  const el = document.createElement('div');
  const gold = Math.random() < 0.15;
  el.className = 'star' + (gold ? ' gold' : '');
  el.textContent = gold ? '🌟' : '⭐';

  const r = skyEl.getBoundingClientRect();
  const s = {
    el,
    x: r.width * (0.06 + Math.random() * 0.88),
    y: -20,
    /* 下落速度加快：约 2.0–4.0 px/帧 */
    vy: 2.0 + Math.random() * 2.0,
    vx: (Math.random() - 0.5) * 0.6,
    gold,
  };
  el.style.transform = `translate(${s.x}px, ${s.y}px)`;
  skyEl.appendChild(el);
  stars.push(s);
}

function tick(ts) {
  if (!state.playing) return;
  if (!lastTs) lastTs = ts;
  const dt = Math.min(40, ts - lastTs);
  lastTs = ts;

  timeLeft -= dt / 1000;
  if (timeLeft < 0) timeLeft = 0;
  timeEl.textContent = `⏱ ${timeLeft.toFixed(1)}s`;

  spawnTimer -= dt;
  if (spawnTimer <= 0) {
    spawnStar();
    /* 生成间隔拉长：约 500–850ms */
    spawnTimer = 500 + Math.random() * 350;
  }

  const r = skyEl.getBoundingClientRect();
  const br = basketEl.getBoundingClientRect();
  const bl = br.left - r.left;
  const brr = br.right - r.left;
  const bt = br.top - r.top;

  stars = stars.filter(s => {
    s.y += s.vy * (dt / 16);
    s.x += s.vx * (dt / 16);
    if (s.x < 8 || s.x > r.width - 8) s.vx *= -1;
    s.el.style.transform = `translate(${s.x}px, ${s.y}px)`;

    /* 接住判定：星星底部进入篮子口范围 */
    if (s.y + 14 >= bt && s.y + 14 <= bt + 28) {
      if (s.x >= bl - 6 && s.x <= brr + 6) {
        s.el.remove();
        score++;
        scoreEl.textContent = `⭐ ${score}`;
        earnEl.textContent = `+${score} 🪙`;
        basketEl.classList.remove('caught');
        void basketEl.offsetWidth;
        basketEl.classList.add('caught');

        /* 接住位置的小飘字 +1 */
        const skyRect = skyEl.getBoundingClientRect();
        spawnFloatText('+1', skyRect.left + s.x, skyRect.top + s.y, 'catch');
        return false;
      }
    }
    if (s.y > r.height + 20) {
      s.el.remove();
      return false;
    }
    return true;
  });

  if (timeLeft <= 0) {
    finish();
    return;
  }
  raf = requestAnimationFrame(tick);
}

/* ============================================================
   金币动画
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
  ], { duration: 1100, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'forwards' })
    .onfinish = () => el.remove();
}

async function flyCoin(fromEl, toEl, count) {
  const fr = fromEl.getBoundingClientRect();
  const tr = toEl.getBoundingClientRect();
  const fx = fr.left + fr.width / 2, fy = fr.top + fr.height / 2;
  const tx = tr.left + tr.width / 2, ty = tr.top + tr.height / 2;
  const promises = [];
  for (let i = 0; i < count; i++) {
    promises.push(new Promise(resolve => {
      setTimeout(() => {
        const c = document.createElement('div');
        c.className = 'coin-sprite';
        c.style.left = fx + 'px';
        c.style.top  = fy + 'px';
        document.body.appendChild(c);
        const dx = tx - fx, dy = ty - fy;
        c.animate([
          { transform: 'translate(-50%, -50%) scale(1) rotate(0deg)', opacity: 1 },
          { transform: `translate(calc(-50% + ${dx*0.5}px), calc(-50% + ${dy*0.5 - 70}px)) scale(1.3) rotate(180deg)`, opacity: 1, offset: .55 },
          { transform: `translate(calc(-50% + ${dx}px), calc(-50% + ${dy}px)) scale(.25) rotate(360deg)`, opacity: 0 }
        ], { duration: 700, easing: 'cubic-bezier(.4, 0, .2, 1)', fill: 'forwards' })
          .onfinish = () => { c.remove(); resolve(); };
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

async function shakeWallet(el, count) {
  const r = el.getBoundingClientRect();
  const fx = r.left + r.width / 2, fy = r.top + r.height / 2;
  el.animate([
    { transform: 'translateX(0)' },
    { transform: 'translateX(-5px) rotate(-2deg)' },
    { transform: 'translateX(5px) rotate(2deg)' },
    { transform: 'translateX(-3px) rotate(-1deg)' },
    { transform: 'translateX(0)' }
  ], { duration: 400, easing: 'cubic-bezier(.4, 0, .2, 1)' });
  const promises = [];
  for (let i = 0; i < count; i++) {
    promises.push(new Promise(resolve => {
      setTimeout(() => {
        const c = document.createElement('div');
        c.className = 'coin-sprite lose';
        c.style.left = fx + 'px';
        c.style.top  = fy + 'px';
        document.body.appendChild(c);
        const a = Math.PI * (0.3 + Math.random() * 0.4);
        const s = 55 + Math.random() * 35;
        const dx = Math.cos(a) * s, dy = -Math.sin(a) * s * 0.7;
        c.animate([
          { transform: 'translate(-50%, -50%) scale(1)', opacity: 1, offset: 0 },
          { transform: `translate(calc(-50% + ${dx}px), calc(-50% + ${dy}px)) scale(1) rotate(140deg)`, opacity: .95, offset: .35 },
          { transform: `translate(calc(-50% + ${dx*1.4}px), calc(-50% + ${dy+90}px)) scale(.35) rotate(340deg)`, opacity: 0 }
        ], { duration: 780, easing: 'cubic-bezier(.4, 0, .6, 1)', fill: 'forwards' })
          .onfinish = () => { c.remove(); resolve(); };
      }, i * 80);
    }));
  }
  await Promise.all(promises);
}

/* ============================================================
   主流程
   ============================================================ */
async function animateStart() {

  state.playing = true;

  /* 扣币 */
  if (state.walletEnabled) {
    // 入场费已由宿主扣除。
    balanceEl.textContent = state.balance;
    balanceEl.classList.remove('up', 'down');
    void balanceEl.offsetWidth;
    balanceEl.classList.add('down');
    setTimeout(() => balanceEl.classList.remove('down'), 900);

    const wr = balanceEl.getBoundingClientRect();
    spawnFloatText(`-${COST}`, wr.left + wr.width / 2, wr.top + wr.height / 2 + 20, 'lose');
  }

  overlay.classList.add('hide');

  score = 0;
  timeLeft = DURATION;
  stars.forEach(s => s.el.remove());
  stars = [];
  spawnTimer = 0;
  lastTs = 0;
  scoreEl.textContent = '⭐ 0';
  earnEl.textContent = '+0 🪙';
  timeEl.textContent = `⏱ ${DURATION.toFixed(1)}s`;

  basketX = 0.5;
  basketEl.style.left = '50%';

  footHint.textContent = '移动鼠标 / 方向键接星星';

  raf = requestAnimationFrame(tick);
}

async function animateFinish() {
  state.playing = false;
  if (raf) { cancelAnimationFrame(raf); raf = null; }

  stars.forEach(s => s.el.remove());
  stars = [];



  /* 每颗 +1 */
  const reward = score;
  const delta = reward - COST;

  showResultPanel();

  if (state.walletEnabled && reward > 0) {
    // 奖励已由宿主结算。
    balanceEl.textContent = state.balance;
    balanceEl.classList.remove('up', 'down');
    void balanceEl.offsetWidth;
    balanceEl.classList.add('up');
    setTimeout(() => balanceEl.classList.remove('up'), 900);

    const sr = skyEl.getBoundingClientRect();
    spawnFloatText(`+${reward}`,
      sr.left + sr.width / 2,
      sr.top + sr.height / 2,
      'gain');
    await flyCoin(skyEl, balanceEl, Math.min(reward, 6));
  }
}

startBtn.addEventListener('click', start);

/* ============================================================
   初始化
   ============================================================ */
(async function init() {
  permBtn.disabled = true;
  try {
    await runtime.initialize();
    if (runtime.progress.activeRound) await runtime.save({ ...runtime.progress, lastRound: { ...runtime.progress.activeRound, abandoned: true }, activeRound: null });
    state.balance = runtime.balance;
    state.walletEnabled = runtime.progress.walletMode !== false && runtime.walletAllowed;
    ready = true; state.busy = false;
    renderMode();

  quotaTotalEl.textContent = `/ ${DAILY_LIMIT}`;
  renderBalance();
  renderQuota();
  showStartPanel();

  } catch (error) { footHint.textContent = FirstPartyRuntime.message(error); }
  finally { permBtn.disabled = !ready; }
})();

async function beginRound() {
  await runtime.refresh(); state.balance = runtime.balance;
  if (state.walletEnabled && !runtime.walletAllowed) throw new Error('钱包权限已撤销，请重新打开游戏。');
  syncQuota();
  if (state.usedToday >= DAILY_LIMIT) throw new Error('今日次数已用完');
  const required = COST;
  if (state.walletEnabled && runtime.balance < BigInt(required)) throw new Error(`喵币不足（需 ${required}）`);
  const roundId = crypto.randomUUID();
  await runtime.save({ ...runtime.progress, day: FirstPartyRuntime.day(), usedToday: state.usedToday + 1 });
  renderQuota();
  return roundId;
}

async function start() {
  if (!canStart()) return;
  state.busy = true; permBtn.disabled = true; refreshOverlay();
  try {
    const roundId = await beginRound();
    const activeRound = { roundId, walletMode: state.walletEnabled, startedAt: Date.now() };
    const after = { ...runtime.progress, activeRound };
    if (state.walletEnabled) await runtime.commit(after, -COST, '接星星：入场费', activeRound);
    else await runtime.save(after);
    state.balance = runtime.balance;
    await animateStart();
    state.busy = false;
  } catch (error) {
    state.busy = false; permBtn.disabled = false; refreshOverlay();
    footHint.textContent = FirstPartyRuntime.message(error);
  }
}
async function finish() {
  if (state.busy) return;
  state.busy = true;
  if (raf) { cancelAnimationFrame(raf); raf = null; }
  try {
    const lastRound = { ...runtime.progress.activeRound, score, reward: score, endedAt: Date.now() };
    const after = { ...runtime.progress, lastRound, activeRound: null };
    if (state.walletEnabled && score > 0) await runtime.commit(after, score, '接星星：完成奖励', lastRound);
    else await runtime.save(after);
    state.balance = runtime.balance;
    await animateFinish();
    state.busy = false; permBtn.disabled = false; refreshOverlay();
  } catch (error) {
    state.playing = false; state.busy = false; permBtn.disabled = false;
    // 无法确认到账时禁止新局；重新打开核对原请求，不产生新奖励请求。
    runtime.blocked = true; overlay.classList.remove('hide'); refreshOverlay();
    footHint.textContent = FirstPartyRuntime.message(error) + '；请重新打开游戏核对。';
  }
}
