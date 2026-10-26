/* 新经典游戏复用已有幂等钱包请求及待核对存档，不维护钱包余额副本。 */
window.ClassicGameRuntime = class ClassicGameRuntime extends window.FirstPartyRuntime {
  constructor(name) {
    super({ day: '', paidRounds: 0, totalPlayed: 0, totalWins: 0, bestScore: 0, current: null }); this.name = name;
  }
  get usedToday() { return this.progress.day === window.FirstPartyRuntime.day() ? this.progress.paidRounds : 0; }
  async begin(game, mode, cost) {
    if (this.progress.current && !this.progress.current.settled) throw new Error('请先完成或结束当前一局。');
    await this.refresh();
    if (!['play', 'earn'].includes(mode)) throw new Error('请选择游玩方式。');
    const progress = structuredClone(this.progress), day = window.FirstPartyRuntime.day();
    if (progress.day !== day) { progress.day = day; progress.paidRounds = 0; }
    if (mode === 'earn') {
      if (!this.walletAllowed) throw new Error('赚钱模式需要喵币权限；也可以选择免费游玩。');
      if (progress.paidRounds >= 5) throw new Error('今天的5次赚钱机会已用完，免费游玩不受限制。');
      if (this.balance < BigInt(cost)) throw new Error('喵币不足，请先签到或选择免费游玩。');
      progress.paidRounds++;
    }
    progress.totalPlayed++;
    progress.current = { ...game, mode, cost: mode === 'earn' ? cost : 0, settled: false, startedAt: Date.now() };
    await this.commit(progress, mode === 'earn' ? -cost : 0, `${this.name}：入场费`, { mode, difficulty: game.level ?? null });
  }
  async update(game, reward = null) {
    if (!this.progress.current || this.progress.current.settled) return;
    const progress = structuredClone(this.progress);
    progress.current = { ...game, mode: this.progress.current.mode, cost: this.progress.current.cost, startedAt: this.progress.current.startedAt, settled: reward !== null };
    progress.bestScore = Math.max(progress.bestScore, game.score ?? 0);
    if (reward !== null) {
      progress.current.endedAt = Date.now();
      if (!Number.isSafeInteger(reward) || reward < 0) throw new Error('结算金额无效。');
      if (game.status === 'won') progress.totalWins++;
      const amount = progress.current.mode === 'earn' ? reward : 0;
      progress.current.reward = amount;
      await this.commit(progress, amount, `${this.name}：完成奖励`, { highest: game.highest ?? null, difficulty: game.level ?? null });
    } else await this.save(progress);
  }
};

/* 只负责展示；不写存档、不触发结算。 */
window.ClassicGameUI = (() => {
  let dialog = null, closeTimer = null, returnFocus = null;
  function close() {
    if (!dialog) return;
    const old = dialog; dialog = null; old.classList.add('closing');
    closeTimer = setTimeout(() => { old.close?.(); old.remove(); returnFocus?.focus?.(); }, 170);
  }
  function show(kind, title, text, label = '知道了') {
    clearTimeout(closeTimer); document.querySelectorAll('.classic-dialog').forEach(node => node.remove());
    returnFocus = document.activeElement; dialog = document.createElement('dialog');
    dialog.className = 'classic-dialog'; dialog.dataset.kind = kind; dialog.setAttribute('aria-labelledby', 'classic-dialog-title');
    const symbol = document.createElement('div'); symbol.className = 'result-symbol'; symbol.setAttribute('aria-hidden', 'true');
    symbol.innerHTML = kind === 'won' ? '<svg viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="2"><circle cx="16" cy="16" r="13"/><path d="m9 16 5 5 9-10"/></svg>' : kind === 'lost' ? '<svg viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="2"><circle cx="16" cy="16" r="13"/><path d="m11 11 10 10m0-10L11 21"/></svg>' : '';
    const heading = document.createElement('h2'); heading.id = 'classic-dialog-title'; heading.textContent = title;
    const body = document.createElement('p'); body.className = 'classic-dialog-body'; body.textContent = text;
    const button = document.createElement('button'); button.type = 'button'; button.className = 'primary'; button.textContent = label; button.onclick = close;
    dialog.append(symbol, heading, body, button); dialog.addEventListener('cancel', event => { event.preventDefault(); close(); });
    if (kind === 'won') for (let i = 0; i < 18; i++) { const piece = document.createElement('i'); piece.className = 'celebration-piece'; piece.setAttribute('aria-hidden', 'true'); piece.style.setProperty('--x', `${(i % 9 - 4) * 32}px`); piece.style.setProperty('--y', `${-35 - i % 5 * 22}px`); piece.style.setProperty('--turn', `${i * 43}deg`); piece.style.setProperty('--delay', `${i % 3 * 35}ms`); dialog.append(piece); }
    document.body.append(dialog); if (dialog.showModal) dialog.showModal(); else dialog.setAttribute('open', ''); button.focus();
  }
  return { show, close };
})();
