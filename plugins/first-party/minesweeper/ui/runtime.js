/* 第一方游戏的数据衔接；钱包余额只来自宿主，存档没有余额副本。 */
window.FirstPartyRuntime = class FirstPartyRuntime {
  constructor(defaultProgress) {
    this.sdk = window.yanmew;
    this.progress = structuredClone(defaultProgress);
    this.envelope = { version: 1, progress: this.progress, pending: null };
    this.balance = 0n;
    this.context = null;
    this.blocked = false;
  }
  static day(now = new Date()) {
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  }
  static message(error) { return error?.message || String(error); }
  async initialize() {
    if (!this.sdk) throw new Error('请从颜喵游戏中心打开此游戏。');
    const saved = await this.sdk.storage.get('state.main');
    if (saved !== null) {
      if (saved.version !== 1 || !saved.progress || typeof saved.progress !== 'object') {
        throw new Error('存档格式不受支持，请保留数据并联系插件作者。');
      }
      this.envelope = saved;
    }
    this.progress = structuredClone(this.envelope.progress);
    await this.refresh();
    await this.recover();
    await this.sdk.game.ready();
    return this.progress;
  }
  async refresh() {
    this.context = await this.sdk.game.getContext();
    const wallet = await this.sdk.wallet.getBalance();
    this.balance = BigInt(wallet.coins);
  }
  get walletAllowed() {
    return this.context?.walletPermissionGranted && this.context.permissions.some(p =>
      p.permission === 'wallet.modify' && p.decision === 'allowed');
  }
  async write(envelope) {
    await this.sdk.storage.set('state.main', envelope);
    this.envelope = structuredClone(envelope);
    this.progress = structuredClone(envelope.progress);
  }
  async save(progress) {
    if (this.blocked || this.envelope.pending) throw new Error('上一笔操作尚未核对，请重新打开游戏恢复。');
    await this.write({ version: 1, progress, pending: null });
  }
  async commit(progress, delta, reason, details = {}, recordZero = false) {
    if (this.blocked || this.envelope.pending) throw new Error('上一笔操作尚未核对，请重新打开游戏恢复。');
    if (delta === 0 && !recordZero) { await this.save(progress); return; }
    if (!this.walletAllowed) throw new Error('钱包权限未开启，请到插件管理中允许后重新打开游戏。');
    const request = {
      request_id: crypto.randomUUID(), game_run_id: this.context.gameRunId,
      delta, reason, details,
    };
    // 同一存档键先保存旧进度与待核对请求，再结算宿主钱包，最后发布新进度。
    await this.write({ version: 1, progress: this.progress, pending: { request, after: progress } });
    await this.settle();
  }
  async settle() {
    const pending = this.envelope.pending;
    try {
      const receipt = await this.sdk.wallet.changeCoins(pending.request);
      this.balance = BigInt(receipt.balance);
      await this.write({ version: 1, progress: pending.after, pending: null });
      this.blocked = false;
    } catch (error) {
      // 以下拒绝发生在 COMMIT 之前；其余错误可能已到账，必须保留请求供重开核对。
      this.blocked = true;
      if (['PLUGIN_PERMISSION_DENIED', 'INSUFFICIENT_COINS', 'COIN_OVERFLOW'].includes(error?.code)) {
        await this.write({ version: 1, progress: this.envelope.progress, pending: null });
        this.blocked = false;
      }
      throw error;
    }
  }
  async recover() {
    if (!this.envelope.pending) return;
    try { await this.settle(); }
    catch (error) {
      // 旧运行未提交的钱包请求已被宿主拒绝；旧进度保留，不重放历史喵币变化。
      if (error?.code !== 'PLUGIN_PERMISSION_DENIED') throw error;
    }
  }
};
