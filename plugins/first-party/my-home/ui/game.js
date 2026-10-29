/* ============================================================
   原文档数据
   ============================================================ */

/* 种子商店 */
const SEEDS = {
  '飞燕草种子':   { name: '飞燕草',   icon: '🌸', buyPrice: 10, sellPrice: null, plantKey: '飞燕草' },
  '可可豆种子':   { name: '可可豆',   icon: '🍫', buyPrice: 6,  sellPrice: 8,    plantKey: '可可豆' },
  '咖啡豆种子':   { name: '咖啡豆',   icon: '☕', buyPrice: 6,  sellPrice: 8,    plantKey: '咖啡豆' },
  '白菜种子':     { name: '白菜',     icon: '🥬', buyPrice: 5,  sellPrice: 7,    plantKey: '白菜' },
  '萝卜种子':     { name: '萝卜',     icon: '🥕', buyPrice: 5,  sellPrice: 7,    plantKey: '萝卜' },
  '花生种子':     { name: '花生',     icon: '🥜', buyPrice: 5,  sellPrice: 7,    plantKey: '花生' },
  '番茄种子':     { name: '番茄',     icon: '🍅', buyPrice: 7,  sellPrice: 9,    plantKey: '番茄' },
  '土豆种子':     { name: '土豆',     icon: '🥔', buyPrice: 7,  sellPrice: 9,    plantKey: '土豆' },
  '辣椒种子':     { name: '辣椒',     icon: '🌶️', buyPrice: 7,  sellPrice: 9,    plantKey: '辣椒' },
};

/* 动物（养殖场商店） */
const ANIMALS = {
  '鸡': { name: '鸡', icon: '🐔', buyPrice: 20, product: '鸡蛋', productIcon: '🥚',
          killYield: { item: '鸡肉', count: 2, sellPrice: 10 } },
  '猪': { name: '猪', icon: '🐷', buyPrice: 50, product: null, productIcon: null,
          killYield: { item: '猪肉', count: 5, sellPrice: 12 } },
  '牛': { name: '牛', icon: '🐄', buyPrice: 70, product: '牛奶', productIcon: '🥛',
          killYield: { item: '牛肉', count: 5, sellPrice: 12 } },
  '羊': { name: '羊', icon: '🐑', buyPrice: 70, product: null, productIcon: null,
          killYield: { item: '羊肉', count: 5, sellPrice: 14 } },
};

/* 商城的「礼物」tab（买回来放进仓库，之后在仓库里送） */
const GIFT_ITEMS = {
  '巧克力':     { icon: '🍫', price: 8,  desc: '最喜欢的食品' },
  '抹茶拿铁':   { icon: '🍵', price: 8,  desc: '最喜欢的饮品' },
  '酒':         { icon: '🍷', price: 10, desc: '一个屑总不可能不喝酒' },
  '毛绒球':     { icon: '🧶', price: 12, desc: '猫猫无法拒绝' },
  '飞燕草':     { icon: '🌸', price: 13, desc: '花语：正义与自由' },
  '毛线球':     { icon: '🧵', price: 12, desc: '猫猫的最爱' },
  '毛绒球挂饰': { icon: '🎀', price: 13, desc: '猫猫最爱的房子' },
};

/* 「送」动作：物品 → 熟练度范围（严格按原文档「送(某物)」） */
const GIFT_SKILL = {
  '巧克力':     [1, 10],
  '抹茶拿铁':   [1, 10],
  '酒':         [5, 15],
  '番茄':       [1, 9],
  '毛绒球':     [8, 18],
  '毛线球':     [8, 18],
  '飞燕草':     [10, 20],
  '毛绒球挂饰': [10, 20],
};

/* 物品卖出价 */
const SELL_PRICE = {
  '可可豆': 8, '咖啡豆': 8,
  '花生': 7, '萝卜': 7, '白菜': 7,
  '番茄': 9, '土豆': 9, '辣椒': 9,
  '飞燕草': null,
  '鸡蛋': 2, '牛奶': 2,
  '鸡肉': 10, '猪肉': 12, '牛肉': 12, '羊肉': 14,
};

const FISH_PER_1000G = 1;

const FARM_OPEN_COST = 50;
const FARM_UPGRADE_BASE_COST = 40;
const FARM_UPGRADE_HIGH_COST = 80;
const FARM_LEVEL_50 = 50;
const FARM_MAX_LEVEL = 120;
const CROP_GROW_HOURS = 1;

const FISH_OPEN_AFFINITY = 30;
const FISH_LOW_COST = 1;
const FISH_HIGH_COST = 2;
const FISH_DAILY_LIMIT = 20;
const FISH_ROD_DURABILITY = 80;
const FISH_ROD_PRICE = 30;
const FISH_LOW_RANGE = [10, 8000];
const FISH_HIGH_RANGE = [20, 15000];

const BARN_OPEN_COST = 60;
const BARN_OPEN_AFFINITY = 500;
const BARN_UPGRADE_COST_LOW = 60;
const BARN_UPGRADE_COST_HIGH = 120;
const BARN_LEVEL_50 = 50;
const BARN_MAX_LEVEL = 120;

const AFFINITY_GAIN = {
  harvest: 1,
  fish: 1,
  animalProduct: 1,
};

/* ============================================================
   状态
   ============================================================ */
let state = {
  coins: 0n,
  affinity: 0,
  inventory: {},

  farm: {
    enabled: false,
    level: 0,
    current: null,
    count: 0,
    readyAt: 0,
    lastDailyReward: '',
  },

  fishing: {
    rodDurability: FISH_ROD_DURABILITY,
    dailyCount: 0,
    lastDay: '',
    fishGrams: 0,
  },

  barn: {
    enabled: false,
    level: 0,
    lastChickenCollect: '',
    lastCowCollect: '',
  },
};

function dateKey() {
  return FirstPartyRuntime.day();
}

/* ============================================================
   DOM
   ============================================================ */
const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];

let currentView = 'home';

function updateCoins() {
  if (draft) return;
  const text = state.coins.toLocaleString('zh-CN');
  $$('.coin-tag .num').forEach(el => { el.textContent = text; });
}

function addCoins(delta, anchor) {
  if (delta === 0) return;
  if (draft) {
    state.coins += BigInt(delta); draft.delta += delta;
    draft.effects.push(() => addCoins(delta, anchor)); return;
  }
  // 成功后只播放原动画，真实余额已经从宿主回执读取。
  updateCoins();
  $$('.coin-tag .num').forEach(el => {
    el.classList.remove('up', 'down', 'pop');
    void el.offsetWidth;
    el.classList.add(delta > 0 ? 'up' : 'down', 'pop');
    setTimeout(() => el.classList.remove('up', 'down'), 900);
  });
  if (anchor) {
    const r = anchor.getBoundingClientRect();
    spawnFloat((delta > 0 ? '+' : '') + delta, r.left + r.width / 2, r.top, delta > 0 ? 'gain' : 'lose');
  }
  renderCurrent();
}

function updateSkill() {
  if (draft) return;
  const text = state.affinity.toLocaleString('zh-CN');
  $$('.skill-tag .num').forEach(el => { el.textContent = text; });
}

function addSkill(delta) {
  if (delta === 0) return;
  if (draft) {
    state.affinity = Math.max(0, state.affinity + delta);
    draft.effects.push(() => addSkill(delta)); return;
  }
  updateSkill();
  $$('.skill-tag .num').forEach(el => {
    el.classList.remove('up', 'down', 'pop');
    void el.offsetWidth;
    el.classList.add(delta > 0 ? 'up' : 'down', 'pop');
    setTimeout(() => el.classList.remove('up', 'down'), 900);
  });
}

function spawnFloat(text, x, y, type = 'gain') {
  if (draft) { draft.effects.push(() => spawnFloat(text, x, y, type)); return; }
  const el = document.createElement('div');
  el.className = 'float-text ' + type;
  el.textContent = text;
  el.style.left = x + 'px';
  el.style.top = y + 'px';
  document.body.appendChild(el);
  el.animate([
    { transform: 'translate(-50%, -50%) scale(.7)', opacity: 0 },
    { transform: 'translate(-50%, -100%) scale(1.15)', opacity: 1, offset: .25 },
    { transform: 'translate(-50%, -220%) scale(1)', opacity: 0 }
  ], { duration: 1100, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'forwards' })
    .onfinish = () => el.remove();
}

let toastTimer;
function toast(msg) {
  if (draft) { draft.effects.push(() => toast(msg)); return; }
  let el = document.querySelector('.toast');
  if (!el) {
    el = document.createElement('div');
    el.className = 'toast';
    document.body.appendChild(el);
  }
  el.textContent = msg;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 2200);
}

/* ============================================================
   背包
   ============================================================ */
function addItem(name, count = 1) {
  state.inventory[name] = (state.inventory[name] || 0) + count;
}
function removeItem(name, count = 1) {
  if ((state.inventory[name] || 0) < count) return false;
  state.inventory[name] -= count;
  if (state.inventory[name] <= 0) delete state.inventory[name];
  return true;
}
function getItem(name) { return state.inventory[name] || 0; }

function itemIcon(name) {
  if (SEEDS[name]) return SEEDS[name].icon;
  if (GIFT_ITEMS[name]) return GIFT_ITEMS[name].icon;
  if (name === '鸡蛋') return '🥚';
  if (name === '牛奶') return '🥛';
  if (name === '鸡肉') return '🍗';
  if (name === '猪肉') return '🥓';
  if (name === '牛肉') return '🥩';
  if (name === '羊肉') return '🍖';
  for (const s of Object.values(SEEDS)) {
    if (s.name === name) return s.icon;
  }
  const r = RECIPES.find(x => x.name === name);
  if (r) return r.icon;
  return '❔';
}

/* ============================================================
   做饭系统
   ============================================================ */
const RECIPES = [
  { name: '番茄炒蛋', icon: '🍳', materials: { '番茄': 1, '鸡蛋': 1 }, sellPrice: 20 },
  { name: '蔬菜拼盘', icon: '🥗', materials: { '白菜': 1, '萝卜': 1, '花生': 1 }, sellPrice: 28 },
  { name: '牛奶炖蛋', icon: '🍮', materials: { '牛奶': 1, '鸡蛋': 1 }, sellPrice: 12 },
  { name: '红烧猪肉', icon: '🥘', materials: { '猪肉': 1 }, sellPrice: 18 },
  { name: '炖牛肉',   icon: '🍲', materials: { '牛肉': 1, '土豆': 1 }, sellPrice: 25 },
];

/* ============================================================
   视图切换
   ============================================================ */
function switchView(name) {
  if (currentView === name) return;
  const oldEl = document.querySelector(`.view[data-view="${currentView}"]`);
  const newEl = document.querySelector(`.view[data-view="${name}"]`);
  if (!newEl) return;
  if (oldEl) {
    oldEl.classList.remove('on');
    oldEl.classList.add('leaving');
    setTimeout(() => oldEl.classList.remove('leaving'), 400);
  }
  newEl.classList.add('on');
  currentView = name;
  renderCurrent();
}

function renderCurrent() {
  if (draft) return;
  if (currentView === 'home')    renderHome();
  if (currentView === 'farm')    renderFarm();
  if (currentView === 'pond')    renderPond();
  if (currentView === 'barn')    renderBarn();
  if (currentView === 'kitchen') renderKitchen();
  if (currentView === 'shop')    renderShop();
  if (currentView === 'storage') renderStorage();
}

document.addEventListener('click', e => {
  const go = e.target.closest('[data-go]');
  if (go) { switchView(go.dataset.go); return; }
  const back = e.target.closest('[data-back]');
  if (back) { switchView('home'); return; }
});

/* ============================================================
   主页
   ============================================================ */
function renderHome() {
  if (draft) return;
  const f = state.farm;
  if (!f.enabled) {
    $('#home-farm-desc').textContent = '未开启 · 需 50 金币';
  } else if (f.current && Date.now() < f.readyAt) {
    const remain = Math.max(0, f.readyAt - Date.now());
    $('#home-farm-desc').textContent = `${SEEDS[f.current].name} 生长中 · ${formatRemain(remain)}`;
  } else if (f.current && Date.now() >= f.readyAt) {
    $('#home-farm-desc').textContent = `${SEEDS[f.current].name} 可收获`;
  } else {
    $('#home-farm-desc').textContent = `等级 ${f.level} · 空闲`;
  }
  const farmBadge = $('#home-farm-badge');
  if (f.enabled && f.current && Date.now() >= f.readyAt) {
    farmBadge.style.display = '';
    farmBadge.textContent = '!';
  } else {
    farmBadge.style.display = 'none';
  }

  const p = state.fishing;
  const canFish = state.affinity >= FISH_OPEN_AFFINITY;
  if (!canFish) {
    $('#home-pond-desc').textContent = '未解锁 · 熟练度不足';
  } else if (p.rodDurability <= 0) {
    $('#home-pond-desc').textContent = '鱼竿已坏';
  } else {
    $('#home-pond-desc').textContent = `鱼竿 ${p.rodDurability}/80 · 今日 ${p.dailyCount}/20`;
  }

  const b = state.barn;
  if (!b.enabled) {
    $('#home-barn-desc').textContent = '未开启 · 需 60 金币';
  } else {
    const parts = [];
    for (const k of ['鸡', '猪', '牛', '羊']) {
      const cnt = getItem(k + '_养殖量');
      if (cnt > 0) parts.push(`${ANIMALS[k].icon}${cnt}`);
    }
    $('#home-barn-desc').textContent = parts.length ? parts.join(' ') : `等级 ${b.level} · 空`;
  }
  const barnBadge = $('#home-barn-badge');
  const hasChickenProduct = b.enabled && getItem('鸡_养殖量') > 0 && b.lastChickenCollect !== dateKey();
  const hasCowProduct = b.enabled && getItem('牛_养殖量') > 0 && b.lastCowCollect !== dateKey();
  if (hasChickenProduct || hasCowProduct) {
    barnBadge.style.display = '';
    barnBadge.textContent = '!';
  } else {
    barnBadge.style.display = 'none';
  }

  const canMake = RECIPES.filter(r => canCraft(r)).length;
  $('#home-kitchen-desc').textContent = canMake > 0 ? `${canMake} 道可做` : `${RECIPES.length} 道菜谱`;
}

function formatRemain(ms) {
  const totalSec = Math.ceil(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}时${m}分`;
  if (m > 0) return `${m}分${s}秒`;
  return `${s}秒`;
}

/* ============================================================
   田地
   ============================================================ */
function renderFarm() {
  if (draft) return;
  const c = $('#farm-container');
  const f = state.farm;

  if (!f.enabled) {
    c.innerHTML = `
      <div class="empty-state">
        <div class="big">🌱</div>
        你还没有田地呢～要去买一块吗？<br>
        开启需要 <strong style="color:var(--gold-deep)">${FARM_OPEN_COST} 金币</strong>
      </div>
      <button class="btn btn-green" id="farm-open">开启种田系统（${FARM_OPEN_COST}🪙）</button>
    `;
    $('#farm-open').addEventListener('click', openFarm);
    return;
  }

  const growing = f.current && Date.now() < f.readyAt;
  const ready = f.current && Date.now() >= f.readyAt;

  let heroHtml = '';
  if (ready) {
    heroHtml = `
      <div class="farm-icon">${SEEDS[f.current].icon}</div>
      <div class="farm-status">已成熟 · ${SEEDS[f.current].name} × ${f.count}</div>
    `;
  } else if (growing) {
    heroHtml = `
      <div class="farm-icon">${SEEDS[f.current].icon}</div>
      <div class="farm-status">${SEEDS[f.current].name} × ${f.count} 生长中</div>
      <div class="farm-progress-wrap"><div class="farm-progress" id="farm-progress"></div></div>
    `;
  } else {
    heroHtml = `
      <div class="farm-icon" style="opacity:.55">🌾</div>
      <div class="farm-status">田地空闲</div>
    `;
  }

  const dailyDone = f.lastDailyReward === dateKey();
  const upgradeCost = f.level >= FARM_LEVEL_50 ? FARM_UPGRADE_HIGH_COST : FARM_UPGRADE_BASE_COST;
  const upgradeAffinityReq = f.level * 50 + 50;
  const canUpgrade = f.level < FARM_MAX_LEVEL && state.coins >= upgradeCost && state.affinity >= upgradeAffinityReq;

  c.innerHTML = `
    <div class="farm-info ${!f.current ? 'empty' : ''}">${heroHtml}</div>

    <div class="farm-stats">
      <div class="stat-box">
        <div class="lbl">田地等级</div>
        <div class="val">${f.level} / ${FARM_MAX_LEVEL}</div>
      </div>
      <div class="stat-box">
        <div class="lbl">单次最大种植</div>
        <div class="val">${f.level} 份</div>
      </div>
    </div>

    <div class="farm-actions">
      ${ready
        ? `<button class="btn btn-gold" id="farm-harvest" style="grid-column:1/-1">🌾 收获 ${f.count} 份</button>`
        : growing
          ? `<button class="btn btn-ghost" disabled style="grid-column:1/-1">生长中…</button>`
          : `<button class="btn btn-green" id="farm-plant" style="grid-column:1/-1">🌱 种植</button>`
      }
      <button class="btn btn-ghost" id="farm-daily" ${dailyDone ? 'disabled' : ''}>
        🎁 每日田地奖励${dailyDone ? '（已领）' : ''}
      </button>
      <button class="btn btn-wood" id="farm-upgrade" ${canUpgrade ? '' : 'disabled'}>
        ⬆ 升级（${upgradeCost}🪙）
      </button>
    </div>

    <div style="font-size:10.5px;color:var(--ink-dim);text-align:center;line-height:1.6;">
      ${f.level >= FARM_LEVEL_50
        ? '50 级后升级需 80 金币'
        : `升级需 ${upgradeCost} 金币 + 熟练度 ${upgradeAffinityReq}`}
      <br>
      熟练度：${state.affinity}${state.affinity < upgradeAffinityReq ? `（还差 ${upgradeAffinityReq - state.affinity}）` : ' ✓'}
    </div>
  `;

  if (ready) {
    $('#farm-harvest').addEventListener('click', harvestFarm);
  } else if (!growing) {
    $('#farm-plant').addEventListener('click', openPlantPicker);
  }
  $('#farm-daily').addEventListener('click', claimDailyReward);
  $('#farm-upgrade').addEventListener('click', upgradeFarm);

  if (growing) updateFarmProgress();
}

function updateFarmProgress() {
  const f = state.farm;
  if (!f.current) return;
  const el = $('#farm-progress');
  if (!el) return;
  const total = CROP_GROW_HOURS * 3600 * 1000;
  const start = f.readyAt - total;
  const elapsed = Date.now() - start;
  const p = Math.max(0, Math.min(1, elapsed / total));
  el.style.width = (p * 100) + '%';
}

function openFarm() {
  if (state.coins < FARM_OPEN_COST) { toast('金币不够哦～再攒攒吧'); return; }
  addCoins(-FARM_OPEN_COST, document.querySelector('#farm-open'));
  state.farm.enabled = true;
  state.farm.level = 1;
  toast('种田系统已开启！');
  renderFarm();
  renderHome();
}

function openPlantPicker() {
  openPickModal({
    title: '选择种子',
    options: Object.keys(SEEDS).map(seedName => {
      const s = SEEDS[seedName];
      const have = getItem(seedName);
      return {
        id: seedName,
        icon: s.icon,
        name: s.name,
        meta: `种子 ×${have} · 种下得 ${s.name}`,
        qty: `×${have}`,
        disabled: have < 1,
      };
    }),
    showNum: true,
    onConfirm: (seedName, num) => plantFarm(seedName, num),
  });
}

function plantFarm(seedName, num) {
  const f = state.farm;
  if (f.current && Date.now() < f.readyAt) { toast('还没收获呢，不能种植'); return; }
  if (f.current && Date.now() >= f.readyAt) { toast('先收获再种植'); return; }
  if (num > f.level) { toast(`田地等级不够，最多能种 ${f.level} 份`); return; }
  if (getItem(seedName) < num) { toast('种子不够哦'); return; }

  removeItem(seedName, num);
  f.current = seedName;
  f.count = num;
  f.readyAt = Date.now() + CROP_GROW_HOURS * 3600 * 1000;

  toast(`已种下 ${SEEDS[seedName].name} ${num} 份！`);
  renderFarm();
  renderHome();
}

function harvestFarm() {
  const f = state.farm;
  if (!f.current || Date.now() < f.readyAt) return;
  const s = SEEDS[f.current];
  const cropName = s.name;
  addItem(cropName, f.count);
  addSkill(AFFINITY_GAIN.harvest);

  spawnFloat(`+${f.count} ${s.icon}`, window.innerWidth / 2, window.innerHeight / 2, 'gold');
  toast(`已收获 ${cropName} ${f.count} 份！`);

  f.current = null;
  f.count = 0;
  f.readyAt = 0;

  renderFarm();
  renderHome();
}

function claimDailyReward() {
  const f = state.farm;
  if (f.lastDailyReward === dateKey()) { toast('今天已经领过了'); return; }

  const n = 1 + Math.floor(Math.random() * 10);
  let msg = '';
  if (n <= 2) {
    addItem('番茄种子', f.level);
    msg = `恭喜！从田地里挖出了 ${f.level} 个番茄种子！！`;
  } else if (n <= 6) {
    addItem('白菜种子', f.level);
    msg = `恭喜！从田地里挖出了 ${f.level} 个白菜种子！！`;
  } else if (n < 10) {
    const coins = f.level * 5;
    addCoins(coins);
    msg = `恭喜！从田地里挖出了 ${coins} 个金币！！`;
  } else {
    const coins = f.level * 10;
    addCoins(coins);
    msg = `恭喜！从田地里挖出了 ${coins} 个金币！今天的运气真的太好了！！`;
  }
  f.lastDailyReward = dateKey();
  toast(msg);
  renderFarm();
  renderHome();
}

function upgradeFarm() {
  const f = state.farm;
  if (f.level >= FARM_MAX_LEVEL) { toast('已经升到最高级了'); return; }
  const cost = f.level >= FARM_LEVEL_50 ? FARM_UPGRADE_HIGH_COST : FARM_UPGRADE_BASE_COST;
  const affinityReq = f.level * 50 + 50;
  if (state.coins < cost) { toast(`金币不够哦，需要 ${cost}`); return; }
  if (state.affinity < affinityReq) { toast(`熟练度不够哦，需要 ${affinityReq}`); return; }

  addCoins(-cost, document.querySelector('#farm-upgrade'));
  const oldLevel = f.level;
  f.level += 1;
  toast(`田地升级！${oldLevel} → ${f.level} 级`);
  renderFarm();
  renderHome();
}

/* ============================================================
   池塘
   ============================================================ */
function renderPond() {
  if (draft) return;
  const c = $('#pond-container');
  const p = state.fishing;

  if (state.affinity < FISH_OPEN_AFFINITY) {
    c.innerHTML = `
      <div class="empty-state">
        <div class="big">🎣</div>
        开启钓鱼功能需要熟练度 ${FISH_OPEN_AFFINITY}<br>
        你当前熟练度 ${state.affinity}，还差 ${FISH_OPEN_AFFINITY - state.affinity}
      </div>
    `;
    return;
  }

  const kg = (p.fishGrams / 1000).toFixed(2);
  const sellValue = Math.floor(p.fishGrams / 1000);
  const rodBroken = p.rodDurability <= 0;
  const dailyDone = p.lastDay === dateKey() && p.dailyCount >= FISH_DAILY_LIMIT;

  c.innerHTML = `
    <div class="pond-area">
      <div class="pond-emoji">${rodBroken ? '🪓' : '🎣'}</div>
      <div class="pond-stats">
        <span class="pond-stat">鱼竿 ${p.rodDurability}/${FISH_ROD_DURABILITY}</span>
        <span class="pond-stat">今日 ${p.dailyCount}/${FISH_DAILY_LIMIT}</span>
      </div>
      <div class="pond-stats">
        <span class="pond-stat">鱼获 ${p.fishGrams} 克（${kg} 千克）</span>
        <span class="pond-stat">可卖 ${sellValue} 🪙</span>
      </div>
    </div>

    <div class="pond-actions">
      <button class="btn btn-ghost" id="pond-low" ${rodBroken || dailyDone ? 'disabled' : ''}>
        🪝 低级塘 <span style="font-family:var(--font-mono);font-size:10px;">${FISH_LOW_COST}🪙</span>
      </button>
      <button class="btn btn-green" id="pond-high" ${rodBroken || dailyDone ? 'disabled' : ''}>
        🎣 高级塘 <span style="font-family:var(--font-mono);font-size:10px;">${FISH_HIGH_COST}🪙</span>
      </button>
    </div>

    ${rodBroken
      ? `<button class="btn btn-gold" id="pond-buy-rod">买鱼竿（${FISH_ROD_PRICE}🪙）</button>`
      : ''
    }

    <button class="btn btn-gold" id="pond-sell" ${p.fishGrams < 1000 ? 'disabled' : ''}>
      💰 卖鱼（${sellValue}🪙）
    </button>

    <div style="font-size:10.5px;color:var(--ink-dim);text-align:center;line-height:1.6;">
      每 1000 克鱼卖 1 金币 · 每天最多钓 ${FISH_DAILY_LIMIT} 次<br>
      鱼竿用 ${FISH_ROD_DURABILITY} 次后会坏掉<br>点按或按住提竿，跟住绿色活动区 · 抛竿扣费，离开算失败
    </div>
  `;

  if (!rodBroken && !dailyDone) {
    $('#pond-low').addEventListener('click', () => doFish('low'));
    $('#pond-high').addEventListener('click', () => doFish('high'));
  }
  if (rodBroken) {
    $('#pond-buy-rod').addEventListener('click', buyRod);
  }
  $('#pond-sell').addEventListener('click', sellFish);
}

let fishingRound = null;
function beginFishingChallenge(grams) {
  const round = { grams, awarded: false };
  fishingRound = round;
  window.HomeFishing.open({
    grams,
    onResult: async caught => {
      if (!caught) return true;
      if (round.awarded) return true;
      if (fishingRound !== round) return false;
      let saved = false;
      await perform('钓鱼收获', () => {
        if (fishingRound !== round || round.awarded) return;
        state.fishing.fishGrams += grams;
        draft.effects.push(() => { round.awarded = true; saved = true; });
      });
      return saved;
    },
    onClose: () => {
      if (fishingRound === round) fishingRound = null;
      if (currentView === 'pond') renderPond();
    },
  });
}
function doFish(tier) {
  if (fishingRound || !window.HomeFishing) return;
  const p = state.fishing;
  const cost = tier === 'low' ? FISH_LOW_COST : FISH_HIGH_COST;
  if (state.affinity < FISH_OPEN_AFFINITY) { toast('熟练度达到30后就能钓鱼啦'); return; }
  if (state.coins < cost) { toast('金币不够哦'); return; }
  if (p.rodDurability <= 0) { toast('鱼竿坏了，先买一根'); return; }
  if (p.lastDay !== dateKey()) { p.lastDay = dateKey(); p.dailyCount = 0; }
  if (p.dailyCount >= FISH_DAILY_LIMIT) { toast('今天已经钓了20次了'); return; }
  const range = tier === 'low' ? FISH_LOW_RANGE : FISH_HIGH_RANGE;
  const grams = Math.floor(range[0] + Math.random() * (range[1] - range[0]));
  addCoins(-cost, tier === 'low' ? $('#pond-low') : $('#pond-high'));
  p.rodDurability -= 1; p.dailyCount += 1; addSkill(AFFINITY_GAIN.fish);
  // 扣费和消耗落盘成功之后才进入挑战；关窗不回滚本次消耗。
  draft.effects.push(() => beginFishingChallenge(grams));
}

function buyRod() {
  if (state.coins < FISH_ROD_PRICE) { toast('金币不够哦'); return; }
  addCoins(-FISH_ROD_PRICE, document.querySelector('#pond-buy-rod'));
  state.fishing.rodDurability = FISH_ROD_DURABILITY;
  toast('已花 30 金币买得鱼竿 ×1');
  renderPond();
  renderHome();
}

function sellFish() {
  const p = state.fishing;
  const kg = Math.floor(p.fishGrams / 1000);
  if (kg < 1) { toast('鱼获不够 1 千克，多钓一些再卖吧'); return; }
  const earned = kg * FISH_PER_1000G;
  p.fishGrams -= kg * 1000;
  addCoins(earned, document.querySelector('#pond-sell'));
  toast(`已卖 ${kg} 千克鱼获，获得 ${earned} 金币！`);
  renderPond();
  renderHome();
}

/* ============================================================
   养殖场
   ============================================================ */
function renderBarn() {
  if (draft) return;
  const c = $('#barn-container');
  const b = state.barn;

  if (!b.enabled) {
    c.innerHTML = `
      <div class="empty-state">
        <div class="big">🐔</div>
        你还没有养殖场呢～要去开启吗<br>
        开启需要 <strong style="color:var(--gold-deep)">${BARN_OPEN_COST} 金币</strong> + 熟练度 ${BARN_OPEN_AFFINITY}<br>
        你当前熟练度 ${state.affinity}
      </div>
      <button class="btn btn-green" id="barn-open">开启养殖场（${BARN_OPEN_COST}🪙）</button>
    `;
    $('#barn-open').addEventListener('click', openBarn);
    return;
  }

  const totalAnimals = ['鸡', '猪', '牛', '羊'].reduce((s, k) => s + getItem(k + '_养殖量'), 0);
  const cost = b.level >= BARN_LEVEL_50 ? BARN_UPGRADE_COST_HIGH : BARN_UPGRADE_COST_LOW;
  const affinityReq = b.level * 100 + 500;
  const canUpgrade = b.level < BARN_MAX_LEVEL && state.coins >= cost && state.affinity >= affinityReq;

  let rowsHtml = '';
  for (const key of ['鸡', '猪', '牛', '羊']) {
    const a = ANIMALS[key];
    const cnt = getItem(key + '_养殖量');
    if (cnt <= 0) continue;

    const hasProduct = a.product && (
      (key === '鸡' && b.lastChickenCollect !== dateKey()) ||
      (key === '牛' && b.lastCowCollect !== dateKey())
    );

    rowsHtml += `
      <div class="row">
        <div class="ico">${a.icon}</div>
        <div class="info">
          <div class="name">${a.name} <span class="count">× ${cnt}</span></div>
          <div class="meta ${hasProduct ? 'ready' : ''}">
            ${a.product ? (hasProduct ? `${a.productIcon} 今日可收` : `${a.productIcon} 今日已收`) : '无产品'}
          </div>
        </div>
        <div class="actions">
          ${a.product ? `<button class="btn btn-green btn-sm" data-collect="${key}" ${hasProduct ? '' : 'disabled'}>收获</button>` : ''}
          <button class="btn btn-danger btn-sm" data-kill="${key}">宰杀</button>
        </div>
      </div>
    `;
  }

  c.innerHTML = `
    <div class="farm-stats">
      <div class="stat-box">
        <div class="lbl">养殖场等级</div>
        <div class="val">${b.level} / ${BARN_MAX_LEVEL}</div>
      </div>
      <div class="stat-box">
        <div class="lbl">总容量 / 已养</div>
        <div class="val">${b.level} / ${totalAnimals}</div>
      </div>
    </div>

    <div class="card-list">
      ${rowsHtml || `<div class="empty-state"><div class="big">🐾</div>还没有养动物，去商城买一只吧</div>`}
    </div>

    <button class="btn btn-wood" id="barn-upgrade" ${canUpgrade ? '' : 'disabled'}>
      ⬆ 升级养殖场（${cost}🪙）
    </button>

    <div style="font-size:10.5px;color:var(--ink-dim);text-align:center;line-height:1.6;">
      ${b.level >= BARN_LEVEL_50
        ? '50 级后升级需 120 金币'
        : `升级需 ${cost} 金币 + 熟练度 ${affinityReq}`}
      <br>
      熟练度：${state.affinity}${state.affinity < affinityReq ? `（还差 ${affinityReq - state.affinity}）` : ' ✓'}
    </div>
  `;

  $('#barn-upgrade').addEventListener('click', upgradeBarn);
  c.querySelectorAll('[data-collect]').forEach(btn => {
    btn.addEventListener('click', () => collectAnimal(btn.dataset.collect));
  });
  c.querySelectorAll('[data-kill]').forEach(btn => {
    btn.addEventListener('click', () => killAnimal(btn.dataset.kill));
  });
}

function openBarn() {
  if (state.coins < BARN_OPEN_COST) { toast('金币不够哦～再攒攒吧'); return; }
  if (state.affinity < BARN_OPEN_AFFINITY) { toast(`熟练度不够，需要 ${BARN_OPEN_AFFINITY}`); return; }
  addCoins(-BARN_OPEN_COST, document.querySelector('#barn-open'));
  state.barn.enabled = true;
  state.barn.level = 1;
  toast('恭喜开启养殖场服务！');
  renderBarn();
  renderHome();
}

function collectAnimal(key) {
  const b = state.barn;
  const cnt = getItem(key + '_养殖量');
  if (cnt <= 0) return;

  if (key === '鸡') {
    if (b.lastChickenCollect === dateKey()) { toast('今天已经捡过了'); return; }
    const got = Math.floor(Math.random() * (cnt + 1));
    b.lastChickenCollect = dateKey();
    if (got > 0) addItem('鸡蛋', got);
    toast(got > 0 ? `已收获鸡蛋 ${got} 个！` : '今天鸡没有下蛋呢…');
  } else if (key === '牛') {
    if (b.lastCowCollect === dateKey()) { toast('今天已经挤过了'); return; }
    const got = Math.floor(Math.random() * (cnt + 1));
    b.lastCowCollect = dateKey();
    if (got > 0) addItem('牛奶', got);
    toast(got > 0 ? `已收获牛奶 ${got} 份！` : '今天牛没有产奶呢…');
  }
  addSkill(AFFINITY_GAIN.animalProduct);
  renderBarn();
  renderHome();
}

function killAnimal(key) {
  const a = ANIMALS[key];
  const cnt = getItem(key + '_养殖量');
  if (cnt <= 0) return;

  openConfirm({
    title: '确认宰杀',
    desc: `宰杀 1 只${a.name}，获得 ${a.killYield.count} 份${a.killYield.item}。`,
    onYes: () => perform('宰杀' + key, () => {
      const keyName = key + '_养殖量';
      if (getItem(keyName) < 1) return;
      state.inventory[keyName] = (state.inventory[keyName] || 0) - 1;
      if (state.inventory[keyName] <= 0) delete state.inventory[keyName];
      addItem(a.killYield.item, a.killYield.count);
      toast(`已宰杀 1 只${a.name}，获得 ${a.killYield.count} 份${a.killYield.item}`);
      renderBarn();
      renderHome();
    })
  });
}

function upgradeBarn() {
  const b = state.barn;
  if (b.level >= BARN_MAX_LEVEL) { toast('已经最高级了'); return; }
  const cost = b.level >= BARN_LEVEL_50 ? BARN_UPGRADE_COST_HIGH : BARN_UPGRADE_COST_LOW;
  const affinityReq = b.level * 100 + 500;
  if (state.coins < cost) { toast(`金币不够，需要 ${cost}`); return; }
  if (state.affinity < affinityReq) { toast(`熟练度不够，需要 ${affinityReq}`); return; }
  addCoins(-cost, document.querySelector('#barn-upgrade'));
  const old = b.level;
  b.level += 1;
  toast(`养殖场升级！${old} → ${b.level} 级`);
  renderBarn();
  renderHome();
}

/* ============================================================
   厨房
   ============================================================ */
function canCraft(r) {
  return Object.entries(r.materials).every(([item, n]) => getItem(item) >= n);
}

function renderKitchen() {
  if (draft) return;
  const list = $('#kitchen-list');
  list.innerHTML = '';
  RECIPES.forEach(r => {
    const ok = canCraft(r);
    const matText = Object.entries(r.materials).map(([item, n]) => {
      const have = getItem(item);
      const color = have >= n ? 'var(--ink-dim)' : 'var(--lose)';
      return `<span style="color:${color}">${itemIcon(item)}×${n}(${have})</span>`;
    }).join(' ');

    const el = document.createElement('div');
    el.className = 'row';
    el.innerHTML = `
      <div class="ico">${r.icon}</div>
      <div class="info">
        <div class="name">${r.name}</div>
        <div class="meta">${matText}</div>
        <div class="meta" style="color:var(--gold-deep);font-weight:700;">卖出 ${r.sellPrice} 🪙</div>
      </div>
      <div class="actions">
        <button class="btn ${ok ? 'btn-gold' : 'btn-ghost'} btn-sm" data-craft="${r.name}" ${ok ? '' : 'disabled'}>做</button>
      </div>
    `;
    list.appendChild(el);
  });

  list.querySelectorAll('[data-craft]').forEach(btn => {
    btn.addEventListener('click', () => craftRecipe(btn.dataset.craft));
  });
}

function craftRecipe(name) {
  const r = RECIPES.find(x => x.name === name);
  if (!r || !canCraft(r)) return;
  Object.entries(r.materials).forEach(([item, n]) => removeItem(item, n));
  addItem(r.name, 1);
  toast(`做好了 1 份${r.name}！`);
  renderKitchen();
  renderHome();
}

/* ============================================================
   商城（3 个 tab）
   ============================================================ */
let shopTab = 'seed';

function renderShop() {
  if (draft) return;
  const list = $('#shop-list');
  list.innerHTML = '';

  if (shopTab === 'seed') {
    for (const [seedName, s] of Object.entries(SEEDS)) {
      const el = document.createElement('div');
      el.className = 'shop-row';
      el.innerHTML = `
        <div class="ico">${s.icon}</div>
        <div class="info">
          <div class="name">${seedName.replace('种子', '')}种子</div>
          <div class="meta">${s.buyPrice}🪙 · 卖出 ${s.sellPrice == null ? '不可卖' : s.sellPrice + '🪙'}</div>
        </div>
        <div class="price">${s.buyPrice}🪙</div>
        <button class="btn btn-wood btn-sm" data-buy-seed="${seedName}" ${state.coins < s.buyPrice ? 'disabled' : ''}>买</button>
      `;
      list.appendChild(el);
    }
  } else if (shopTab === 'animal') {
    for (const [animalName, a] of Object.entries(ANIMALS)) {
      const el = document.createElement('div');
      el.className = 'shop-row';
      el.innerHTML = `
        <div class="ico">${a.icon}</div>
        <div class="info">
          <div class="name">${animalName}</div>
          <div class="meta">${a.product ? `产出 ${a.product}` : '仅供宰杀'}</div>
        </div>
        <div class="price">${a.buyPrice}🪙</div>
        <button class="btn btn-wood btn-sm" data-buy-animal="${animalName}" ${state.coins < a.buyPrice ? 'disabled' : ''}>买</button>
      `;
      list.appendChild(el);
    }
  } else if (shopTab === 'gift') {
    for (const [giftName, g] of Object.entries(GIFT_ITEMS)) {
      const skillRange = GIFT_SKILL[giftName];
      const el = document.createElement('div');
      el.className = 'shop-row';
      el.innerHTML = `
        <div class="ico">${g.icon}</div>
        <div class="info">
          <div class="name">${giftName}</div>
          <div class="meta">${g.desc}</div>
          <div class="meta gain">✨ 送出去 +${skillRange[0]}~${skillRange[1]} 熟练度</div>
        </div>
        <div class="price">${g.price}🪙</div>
        <button class="btn btn-wood btn-sm" data-buy-gift="${giftName}" ${state.coins < g.price ? 'disabled' : ''}>买</button>
      `;
      list.appendChild(el);
    }
  }

  list.querySelectorAll('[data-buy-seed]').forEach(btn => {
    btn.addEventListener('click', () => buySeed(btn.dataset.buySeed));
  });
  list.querySelectorAll('[data-buy-animal]').forEach(btn => {
    btn.addEventListener('click', () => buyAnimal(btn.dataset.buyAnimal));
  });
  list.querySelectorAll('[data-buy-gift]').forEach(btn => {
    btn.addEventListener('click', () => buyGiftItem(btn.dataset.buyGift));
  });
}

function buySeed(seedName) {
  const s = SEEDS[seedName];
  if (state.coins < s.buyPrice) return;
  addCoins(-s.buyPrice, document.querySelector(`[data-buy-seed="${seedName}"]`));
  addItem(seedName, 1);
  renderShop();
}

function buyAnimal(animalName) {
  const a = ANIMALS[animalName];
  if (state.coins < a.buyPrice) return;
  const b = state.barn;
  if (!b.enabled) { toast('先开启养殖场'); return; }
  const total = ['鸡', '猪', '牛', '羊'].reduce((s, k) => s + getItem(k + '_养殖量'), 0);
  if (total >= b.level) { toast(`养殖场已满（容量 ${b.level}）`); return; }
  addCoins(-a.buyPrice, document.querySelector(`[data-buy-animal="${animalName}"]`));
  addItem(animalName + '_养殖量', 1);
  toast(`已购买 1 只${animalName}`);
  renderShop();
  renderHome();
}

function buyGiftItem(giftName) {
  const g = GIFT_ITEMS[giftName];
  if (state.coins < g.price) return;
  addCoins(-g.price, document.querySelector(`[data-buy-gift="${giftName}"]`));
  addItem(giftName, 1);
  toast(`已购买 1 份${giftName}，去仓库送出吧～`);
  renderShop();
}

document.querySelectorAll('.shop-tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.shop-tab').forEach(t => t.classList.toggle('on', t === tab));
    shopTab = tab.dataset.shopTab;
    renderShop();
  });
});

/* ============================================================
   仓库
   ============================================================ */
function renderStorage() {
  if (draft) return;
  const list = $('#storage-list');
  list.innerHTML = '';

  const entries = Object.entries(state.inventory).filter(([_, n]) => n > 0);
  const hasFish = state.fishing.fishGrams >= 1000;

  if (entries.length === 0 && !hasFish) {
    list.innerHTML = `
      <div class="empty-state">
        <div class="big">🎒</div>
        仓库空空的<br>
        去种点菜或钓点鱼吧
      </div>
    `;
    return;
  }

  if (hasFish) {
    const kg = Math.floor(state.fishing.fishGrams / 1000);
    const el = document.createElement('div');
    el.className = 'shop-row';
    el.innerHTML = `
      <div class="ico">🐟</div>
      <div class="info">
        <div class="name">鱼获</div>
        <div class="meta">${state.fishing.fishGrams} 克 · 每 1000 克 1🪙</div>
      </div>
      <span class="qty">${kg} kg</span>
      <div class="row-actions">
        <button class="btn btn-gold btn-xs" data-sell="fish">卖</button>
      </div>
    `;
    list.appendChild(el);
  }

  const itemEntries = entries.filter(([name]) => {
    return SELL_PRICE[name] !== undefined ||
           name.endsWith('种子') ||
           name.endsWith('_养殖量') ||
           RECIPES.some(r => r.name === name) ||
           GIFT_SKILL[name];
  });

  const groups = {
    '种子': [], '作物': [], '礼物': [], '蛋奶': [], '肉类': [], '菜品': [], '动物': [],
  };
  for (const [name, count] of itemEntries) {
    if (name.endsWith('_养殖量')) { groups['动物'].push([name, count]); continue; }
    if (name.endsWith('种子')) { groups['种子'].push([name, count]); continue; }
    if (RECIPES.some(r => r.name === name)) { groups['菜品'].push([name, count]); continue; }
    if (name === '鸡蛋' || name === '牛奶') { groups['蛋奶'].push([name, count]); continue; }
    if (['鸡肉', '猪肉', '牛肉', '羊肉'].includes(name)) { groups['肉类'].push([name, count]); continue; }
    // 飞燕草放进「礼物」组，因为它的主用途是送
    if (name === '飞燕草') { groups['礼物'].push([name, count]); continue; }
    // 巧克力/抹茶拿铁/酒/毛绒球/毛线球/毛绒球挂饰
    if (GIFT_ITEMS[name]) { groups['礼物'].push([name, count]); continue; }
    groups['作物'].push([name, count]);
  }

  const ORDER = ['作物', '礼物', '蛋奶', '肉类', '菜品', '种子', '动物'];

  for (const groupName of ORDER) {
    const items = groups[groupName];
    if (!items || items.length === 0) continue;

    const header = document.createElement('div');
    header.style.cssText = 'font-size:10.5px;color:var(--ink-dim);letter-spacing:.6px;text-transform:uppercase;padding:8px 4px 2px;font-weight:700;';
    header.textContent = groupName;
    list.appendChild(header);

    for (const [name, count] of items) {
      if (name.endsWith('_养殖量')) {
        const an = name.replace('_养殖量', '');
        const el = document.createElement('div');
        el.className = 'shop-row';
        el.innerHTML = `
          <div class="ico">${ANIMALS[an].icon}</div>
          <div class="info">
            <div class="name">${an}</div>
            <div class="meta">养殖场中 · 不可卖</div>
          </div>
          <span class="qty">×${count}</span>
        `;
        list.appendChild(el);
        continue;
      }

      const price = SELL_PRICE[name] ?? RECIPES.find(recipe => recipe.name === name)?.sellPrice;
      const canSell = price !== undefined && price !== null;
      const skillRange = GIFT_SKILL[name];
      const canSend = !!skillRange;

      // meta 描述
      let metaParts = [];
      if (canSell) metaParts.push(`卖出 ${price}🪙 / 份`);
      if (canSend) metaParts.push(`送出去 +${skillRange[0]}~${skillRange[1]} 熟练度`);
      if (metaParts.length === 0) metaParts.push('不可卖');
      const metaText = metaParts.join(' · ');

      const el = document.createElement('div');
      el.className = 'shop-row';
      el.innerHTML = `
        <div class="ico">${itemIcon(name)}</div>
        <div class="info">
          <div class="name">${name}</div>
          <div class="meta ${canSend ? 'gain' : ''}">${metaText}</div>
        </div>
        <span class="qty">×${count}</span>
        <div class="row-actions">
          ${canSell ? `<button class="btn btn-gold btn-xs" data-sell="${name}">卖</button>` : ''}
          ${canSend ? `<button class="btn btn-green btn-xs" data-send="${name}">送</button>` : ''}
        </div>
      `;
      list.appendChild(el);
    }
  }

  list.querySelectorAll('[data-sell]').forEach(btn => {
    btn.addEventListener('click', () => sellItem(btn.dataset.sell));
  });
  list.querySelectorAll('[data-send]').forEach(btn => {
    btn.addEventListener('click', () => sendGift(btn.dataset.send));
  });
}

function sellItem(name) {
  if (name === 'fish') {
    const kg = Math.floor(state.fishing.fishGrams / 1000);
    if (kg < 1) return;
    const earned = kg;
    state.fishing.fishGrams -= kg * 1000;
    addCoins(earned, document.querySelector('[data-sell="fish"]'));
    toast(`已卖 ${kg} 千克鱼，获得 ${earned} 金币`);
    renderStorage();
    renderHome();
    return;
  }

  const price = SELL_PRICE[name] ?? RECIPES.find(recipe => recipe.name === name)?.sellPrice;
  if (price === undefined || price === null) return;
  if (getItem(name) < 1) return;

  removeItem(name, 1);
  addCoins(price, document.querySelector(`[data-sell="${name}"]`));
  renderStorage();
  renderHome();
}

/* 送：消耗 1 份，加熟练度（严格按原文档「送(某物)」） */
function sendGift(name) {
  const range = GIFT_SKILL[name];
  if (!range) return;
  if (getItem(name) < 1) return;

  removeItem(name, 1);
  const [min, max] = range;
  const gain = min + Math.floor(Math.random() * (max - min + 1));
  addSkill(gain);

  const btn = document.querySelector(`[data-send="${name}"]`);
  if (btn) {
    const r = btn.getBoundingClientRect();
    spawnFloat(`+${gain} ✨`, r.left + r.width / 2, r.top - 12, 'skill');
  }

  toast(`送出了${name}，熟练度 +${gain}！`);
  renderStorage();
  renderHome();
}

/* ============================================================
   弹窗
   ============================================================ */
let selectedPickId = null;
let pickOnConfirm = null;
let pickShowNum = false;

function openPickModal({ title, options, showNum = false, onConfirm }) {
  $('#pick-title').textContent = title;
  const list = $('#pick-list');
  list.innerHTML = '';
  selectedPickId = null;
  pickOnConfirm = onConfirm;
  pickShowNum = showNum;

  options.forEach(opt => {
    const el = document.createElement('div');
    el.className = 'opt' + (opt.disabled ? ' disabled' : '');
    el.innerHTML = `
      <span class="ico">${opt.icon}</span>
      <div class="info">
        <div class="name">${opt.name}</div>
        <div class="meta">${opt.meta || ''}</div>
      </div>
      <span class="qty">${opt.qty || ''}</span>
    `;
    if (!opt.disabled) {
      el.addEventListener('click', () => {
        list.querySelectorAll('.opt').forEach(x => x.style.borderColor = '');
        el.style.borderColor = 'var(--green)';
        selectedPickId = opt.id;
        if (pickShowNum) {
          const numInput = $('#pick-num');
          numInput.value = 1;
          numInput.max = state.farm.level || 99;
        }
      });
    }
    list.appendChild(el);
  });

  $('#pick-num-wrap').style.display = showNum ? 'flex' : 'none';

  const confirmBtn = $('#pick-confirm');
  confirmBtn.disabled = false;
  confirmBtn.onclick = () => {
    if (!selectedPickId) { toast('请先选择'); return; }
    const num = pickShowNum ? Math.max(1, parseInt($('#pick-num').value) || 1) : 1;
    closeModal($('#pick-modal'));
    pickOnConfirm && pickOnConfirm(selectedPickId, num);
  };

  openModal($('#pick-modal'));
}

function openModal(el) { el.classList.add('on'); }
let confirmCleanup = null;
function closeModal(el) {
  el.classList.remove('on');
  if (el.id === 'confirm-modal' && confirmCleanup) {
    confirmCleanup(); confirmCleanup = null;
  }
}

function openConfirm({ title, desc, onYes }) {
  closeModal($('#confirm-modal'));
  $('#confirm-title').textContent = title;
  $('#confirm-desc').textContent = desc;
  openModal($('#confirm-modal'));

  const yes = $('#confirm-modal [data-confirm-yes]');
  const no = $('#confirm-modal [data-confirm-no]');
  const cleanup = () => {
    yes.removeEventListener('click', hy);
    no.removeEventListener('click', hn);
  };
  const hy = () => { cleanup(); closeModal($('#confirm-modal')); onYes(); };
  const hn = () => { cleanup(); closeModal($('#confirm-modal')); };
  confirmCleanup = cleanup;
  yes.addEventListener('click', hy);
  no.addEventListener('click', hn);
}

document.addEventListener('click', e => {
  if (e.target.closest('[data-modal-close]')) {
    closeModal($('#pick-modal'));
    closeModal($('#confirm-modal'));
  }
  if (e.target.id === 'pick-modal') closeModal($('#pick-modal'));
  if (e.target.id === 'confirm-modal') closeModal($('#confirm-modal'));
});

/* ============================================================
   帧循环
   ============================================================ */
let lastTick = performance.now();

function loop(now) {
  const dt = Math.min(200, now - lastTick);
  lastTick = now;

  const f = state.farm;
  if (f.enabled && f.current && Date.now() >= f.readyAt) {
    if (currentView === 'farm') {
      if (!document.querySelector('#farm-harvest')) renderFarm();
    }
    if (currentView === 'home') renderHome();
  } else if (f.enabled && f.current) {
    if (currentView === 'farm') updateFarmProgress();
    if (currentView === 'home' && now % 1000 < 20) renderHome();
  }

  if (!actionBusy && state.fishing.lastDay !== dateKey()) state.fishing.dailyCount = 0;

  requestAnimationFrame(loop);
}

/* ============================================================
   初始化
   ============================================================ */

// 将原型同步动作作为草稿执行，落盘成功后再公开库存与原动画。
const initialProgress = (() => { const {coins, ...progress} = state; return progress; })();
const runtime = new FirstPartyRuntime(initialProgress);
let initialized = false;
let actionBusy = false;
let draft = null;
function progressSnapshot() { const {coins, ...progress} = state; return structuredClone(progress); }
async function perform(reason, action) {
  if (!initialized || actionBusy || runtime.blocked) return;
  actionBusy = true;
  let before;
  try {
    await runtime.refresh();
    if (!runtime.walletAllowed) throw new Error('我的家需要钱包授权，请到插件管理允许后重新打开。');
    state.coins = runtime.balance;
    before = state;
    state = structuredClone(state);
    draft = {delta: 0, effects: []};
    action();
    const after = progressSnapshot();
    const staged = draft;
    draft = null; state = before;
    await runtime.commit(after, staged.delta, '我的家：' + reason, {action: reason});
    state = {...runtime.progress, coins: runtime.balance};
    updateCoins(); updateSkill(); renderCurrent(); renderHome();
    staged.effects.forEach(effect => effect());
  } catch (error) {
    draft = null;
    if (before) state = before;
    toast(FirstPartyRuntime.message(error) + (runtime.blocked ? '；请重新打开核对存档。' : ''));
  } finally { actionBusy = false; }
}
// 复用原型规则；所有改变私有进度的入口都统一提交，避免快速点击重复消费。
const original_openFarm = openFarm;
openFarm = (...args) => perform('开启田地' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_openFarm(...args));
const original_plantFarm = plantFarm;
plantFarm = (...args) => perform('种植' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_plantFarm(...args));
const original_harvestFarm = harvestFarm;
harvestFarm = (...args) => perform('收获作物' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_harvestFarm(...args));
const original_claimDailyReward = claimDailyReward;
claimDailyReward = (...args) => perform('领取田地奖励' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_claimDailyReward(...args));
const original_upgradeFarm = upgradeFarm;
upgradeFarm = (...args) => perform('升级田地' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_upgradeFarm(...args));
const original_doFish = doFish;
doFish = (...args) => perform('钓鱼' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_doFish(...args));
const original_buyRod = buyRod;
buyRod = (...args) => perform('购买鱼竿' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_buyRod(...args));
const original_sellFish = sellFish;
sellFish = (...args) => perform('出售鱼获' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_sellFish(...args));
const original_openBarn = openBarn;
openBarn = (...args) => perform('开启养殖场' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_openBarn(...args));
const original_collectAnimal = collectAnimal;
collectAnimal = (...args) => perform('收获蛋奶' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_collectAnimal(...args));
const original_upgradeBarn = upgradeBarn;
upgradeBarn = (...args) => perform('升级养殖场' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_upgradeBarn(...args));
const original_craftRecipe = craftRecipe;
craftRecipe = (...args) => perform('制作菜品' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_craftRecipe(...args));
const original_buySeed = buySeed;
buySeed = (...args) => perform('购买种子' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_buySeed(...args));
const original_buyAnimal = buyAnimal;
buyAnimal = (...args) => perform('购买动物' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_buyAnimal(...args));
const original_buyGiftItem = buyGiftItem;
buyGiftItem = (...args) => perform('购买礼物' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_buyGiftItem(...args));
const original_sellItem = sellItem;
sellItem = (...args) => perform('出售物品' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_sellItem(...args));
const original_sendGift = sendGift;
sendGift = (...args) => perform('送出礼物' + (typeof args[0] === 'string' ? '（' + args[0] + '）' : ''), () => original_sendGift(...args));
(async function init() {
  try {
    await runtime.initialize();
    if (!runtime.walletAllowed) throw new Error('我的家需要钱包授权，请到插件管理允许后重新打开。');
    state = {...runtime.progress, coins: runtime.balance};
    initialized = true;
    updateCoins(); updateSkill(); renderHome();
    requestAnimationFrame(loop);
  } catch (error) { toast(FirstPartyRuntime.message(error)); }
})();
window.addEventListener('focus', async () => {
  if (!initialized || actionBusy) return;
  try { await runtime.refresh(); if (actionBusy) return; state.coins = runtime.balance; updateCoins(); renderCurrent(); }
  catch (error) { toast(FirstPartyRuntime.message(error)); }
});
