
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
    syncQuota(); renderMode(); renderQuota(); refreshButtons();
  } catch (error) { footHint.textContent = FirstPartyRuntime.message(error); }
}
window.addEventListener('focus', refreshWallet);
setInterval(refreshWallet, 60000);
/* ============================================================
   常量
   ============================================================ */
const CHOICES = ['rock', 'scissors', 'paper'];
const EMOJI = { rock: '✊', scissors: '✌️', paper: '✋' };
const REWARD = { win: 5, lose: -4, draw: 0 };
const MIN_BET = 4;
const DAILY_LIMIT = 20;

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

const userHandEl  = $('#userHand');
const aiHandEl    = $('#aiHand');
const countdownEl = $('#countdown');

const resultArea  = $('#resultArea');
const stampEl     = $('#stamp');
const stampTextEl = $('#stampText');
const stampDeltaEl= $('#stampDelta');

const actionBtns  = [...document.querySelectorAll('.hand-btn')];
const footHint    = $('#footHint');

const sleep = ms => new Promise(r => setTimeout(r, ms));

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
function refreshButtons() {
  const ok = canPlay();
  actionBtns.forEach(b => { b.disabled = !ok; });
  if (state.usedToday >= DAILY_LIMIT) {
    footHint.textContent = '今天玩够啦，明天再来';
  } else if (state.walletEnabled && state.balance < MIN_BET) {
    footHint.textContent = `至少需要 ${MIN_BET} 喵币才能下注`;
  } else {
    footHint.textContent = '点击任意手势开始';
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
    renderMode(); refreshButtons();
  } catch (error) { errorText = FirstPartyRuntime.message(error); }
  finally { state.busy = false; permBtn.disabled = false; refreshButtons(); if (errorText) footHint.textContent = errorText; }
});

/* ============================================================
   倒计时
   ============================================================ */
async function runCountdown() {
  for (const w of ['石头', '剪刀', '布']) {
    countdownEl.textContent = w;
    countdownEl.classList.remove('pop');
    void countdownEl.offsetWidth;
    countdownEl.classList.add('pop');
    await sleep(280);
  }
  await sleep(120);
  countdownEl.classList.remove('pop');
  countdownEl.textContent = '';
}

/* ============================================================
   判定
   ============================================================ */
function judge(user, ai) {
  if (user === ai) return 'draw';
  if (
    (user === 'rock' && ai === 'scissors') ||
    (user === 'scissors' && ai === 'paper') ||
    (user === 'paper' && ai === 'rock')
  ) return 'win';
  return 'lose';
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
    const d = REWARD[result];
    deltaText = d > 0 ? `+${d}` : d < 0 ? `${d}` : '±0';
  }
  stampDeltaEl.textContent = deltaText;

  stampEl.classList.add(result, 'show');
}

/* ============================================================
   金币获得动画：从结果区弧线飞向钱包
   ============================================================ */
async function playGainAnimation(fromEl, toEl, count) {
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
        coin.className = 'coin-sprite';
        coin.style.left = fx + 'px';
        coin.style.top  = fy + 'px';
        document.body.appendChild(coin);

        const dx = tx - fx;
        const dy = ty - fy;
        const arcX = dx * 0.5;
        const arcY = dy * 0.5 - 70;

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

  // 金币落入时钱包弹跳
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
   金币失去动画：钱包抖动 + 金币向外喷出并下坠
   ============================================================ */
async function playLoseAnimation(fromEl, count) {
  const rect = fromEl.getBoundingClientRect();
  const fx = rect.left + rect.width / 2;
  const fy = rect.top  + rect.height / 2;

  // 钱包抖动
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

        // 向外上方喷出，然后受重力下落
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
   结算
   ============================================================ */
async function applyReward(result) {
  if (!state.walletEnabled) return;

  const d = REWARD[result];
  if (d === 0) return;

  // 数值已由宿主结算，此处只保留原动画。

  balanceEl.textContent = state.balance;
  balanceEl.classList.remove('up', 'down');
  void balanceEl.offsetWidth;
  balanceEl.classList.add(d > 0 ? 'up' : 'down');
  setTimeout(() => balanceEl.classList.remove('up', 'down'), 900);

  const resultRect = resultArea.getBoundingClientRect();
  const walletRect = balanceEl.getBoundingClientRect();

  if (d > 0) {
    // 获得：从结果区飞向钱包，飘出 +N
    spawnFloatText(`+${d}`,
      resultRect.left + resultRect.width / 2,
      resultRect.top  + resultRect.height / 2 - 20,
      'gain');
    await playGainAnimation(resultArea, balanceEl, Math.min(d, 5));
  } else {
    // 失去：钱包抖动，金币从钱包喷出掉落，飘出 −N
    spawnFloatText(`${d}`,
      walletRect.left + walletRect.width / 2,
      walletRect.top  + walletRect.height / 2 + 20,
      'lose');
    await playLoseAnimation(balanceEl, Math.min(Math.abs(d), 4));
  }
}

/* ============================================================
   主流程
   ============================================================ */
async function animatePlay(userChoice, aiChoice) {

  state.busy = true;
  refreshButtons();



  // 清上一局
  stampEl.classList.remove('show', 'win', 'lose', 'draw');
  stampTextEl.textContent = '——';
  stampDeltaEl.textContent = '';
  aiHandEl.textContent = '❔';
  aiHandEl.classList.remove('pop', 'waiting');

  // 用户出拳
  userHandEl.textContent = EMOJI[userChoice];
  userHandEl.classList.remove('waiting', 'pop', 'shake');
  void userHandEl.offsetWidth;
  userHandEl.classList.add('pop');

  // AI 等待闪烁
  aiHandEl.classList.add('waiting');

  await runCountdown();

  // AI 揭晓
  aiHandEl.classList.remove('waiting');
  aiHandEl.textContent = EMOJI[aiChoice];
  void aiHandEl.offsetWidth;
  aiHandEl.classList.add('pop');

  await sleep(300);

  const result = judge(userChoice, aiChoice);

  if (result === 'draw') {
    userHandEl.classList.add('shake');
    aiHandEl.classList.add('shake');
    setTimeout(() => {
      userHandEl.classList.remove('shake');
      aiHandEl.classList.remove('shake');
    }, 470);
  }

  showStamp(result);

  await sleep(340);

  await applyReward(result);



  await sleep(600);

  state.busy = false;
  refreshButtons();
}

/* ============================================================
   绑定
   ============================================================ */
actionBtns.forEach(btn => {
  btn.addEventListener('click', () => play(btn.dataset.choice));
});

window.addEventListener('keydown', e => {
  if (e.repeat) return;
  const map = { '1': 'scissors', '2': 'rock', '3': 'paper' };
  const c = map[e.key];
  if (!c) return;
  if (!canPlay()) return;
  play(c);
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
  renderBalance();
  renderQuota();
  refreshButtons();

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

async function play(userChoice) {
  if (!canPlay()) return;
  state.busy = true; permBtn.disabled = true; refreshButtons();
  let errorText = '';
  try {
    const roundId = await beginRound();
    const aiChoice = CHOICES[Math.floor(await runtime.sdk.game.random() * CHOICES.length)];
    const result = judge(userChoice, aiChoice);
    const after = { ...runtime.progress, lastRound: { roundId, userChoice, aiChoice, result, walletMode: state.walletEnabled } };
    if (state.walletEnabled) await runtime.commit(after, REWARD[result], '剪刀石头布：' + ({win:'胜利',lose:'失败',draw:'平局'}[result]), after.lastRound, true);
    else await runtime.save(after);
    state.balance = runtime.balance;
    await animatePlay(userChoice, aiChoice);
  } catch (error) { errorText = FirstPartyRuntime.message(error); }
  finally {
    state.busy = false; permBtn.disabled = false; renderQuota(); refreshButtons();
    if (errorText) footHint.textContent = errorText;
  }
}
