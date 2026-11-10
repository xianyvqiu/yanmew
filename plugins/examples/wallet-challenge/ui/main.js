const api = window.yanmew;
const element = id => document.querySelector(`#${id}`);
const ui = Object.fromEntries(["balance", "progress", "start", "answer-form", "question", "answer", "submit", "status", "retry", "close"].map(id => [id, element(id)]));
const key = "state.main";
const clone = value => structuredClone(value);
const message = error => error?.message ?? String(error);
let context = null;
let state = null;
let busy = true;
function allowed() {
  return context?.walletPermissionGranted && context.permissions.some(p => p.permission === "wallet.modify" && p.scope === "" && p.decision === "allowed");
}
function progressValid(p) {
  return p && Number.isSafeInteger(p.rounds) && p.rounds >= 0 && Number.isSafeInteger(p.wins) && p.wins >= 0 && p.wins <= p.rounds &&
    (p.round === null || (typeof p.round?.id === "string" && /^[a-z0-9-]{1,64}$/.test(p.round.id) && [p.round.left, p.round.right].every(n => Number.isInteger(n) && n >= 1 && n <= 9)));
}
function checked(value) {
  if (value === null) return {version: 1, progress: {rounds: 0, wins: 0, round: null}, pending: null};
  const p = value?.pending;
  if (value.version !== 1 || !progressValid(value.progress) || (p !== null && (!p || !progressValid(p.nextProgress) ||
      !p.request || !["entry", "reward"].includes(p.kind) || p.request.delta !== (p.kind === "entry" ? -1 : 2) ||
      typeof p.request.request_id !== "string" || p.request.request_id.length > 128 ||
      typeof p.request.game_run_id !== "string" || !/^\d+$/.test(p.request.game_run_id) ||
      typeof p.request.reason !== "string" || !p.request.reason.trim()))) {
    throw new Error("存档格式不兼容，未覆盖原存档；请联系作者。");
  }
  return clone(value);
}
function render() {
  const pending = state?.pending;
  const round = state?.progress.round;
  ui.progress.textContent = state ? `已开始 ${state.progress.rounds} 局，答对 ${state.progress.wins} 局。` : "进度暂不可用。";
  ui.question.textContent = round ? `${round.left} + ${round.right} = ?` : "尚未开始";
  ui.start.disabled = busy || !allowed() || !state || !!pending || !!round;
  ui.answer.disabled = ui.submit.disabled = busy || !allowed() || !round || !!pending;
  ui.retry.hidden = !!state && !pending;
  ui.retry.disabled = busy || !api;
  ui.close.disabled = busy || !api;
}
async function save(next) {
  await api.storage.set(key, next);
  state = clone(next);
}
async function load() {
  state = checked(await api.storage.get(key));
}
async function settle() {
  const pending = state.pending;
  if (!pending) return;
  // 使用存档中的完整原请求；重开后不能替换 game_run_id 或 request_id。
  const result = await api.wallet.changeCoins(pending.request);
  await save({version: 1, progress: pending.nextProgress, pending: null});
  ui.balance.textContent = `余额：${result.balance} 喵币`;
  ui.status.textContent = pending.kind === "entry" ? "入场已确认，请计算答案。" : "奖励已确认，已计入本局进度。";
}
async function prepare(kind, nextProgress, roundId) {
  const request = {
    request_id: `${kind}.${roundId}`,
    game_run_id: context.gameRunId,
    delta: kind === "entry" ? -1 : 2,
    reason: kind === "entry" ? "开发示例：计算挑战入场" : "开发示例：答对计算挑战"
  };
  // 钱包与存档不是同一事务：先持久化请求及目标进度，再请求钱包。
  await save({...state, pending: {kind, request, nextProgress}});
  await settle();
}
async function run(operation) {
  if (busy) return;
  busy = true;
  render();
  try { await operation(); }
  catch (error) {
    // 写入响应丢失也可能已经落盘，先重读，避免用旧内存创建另一笔结算。
    try { await load(); } catch { state = null; }
    ui.status.textContent = state?.pending
      ? `结算尚未确认：${message(error)}。请重新核对；持续失败时联系作者并核对账单，不清空存档或重新发奖。`
      : `操作未完成：${message(error)}。`;
  } finally { busy = false; render(); }
}
ui.start.addEventListener("click", () => run(async () => {
  context = await api.game.getContext();
  if (!allowed()) throw new Error("请在插件详情允许喵币权限，并重新打开游戏。");
  if (state.pending || state.progress.round) return;
  const round = {id: crypto.randomUUID(), left: 1 + Math.floor(await api.game.random() * 9), right: 1 + Math.floor(await api.game.random() * 9)};
  await prepare("entry", {...state.progress, rounds: state.progress.rounds + 1, round}, round.id);
}));
ui["answer-form"].addEventListener("submit", event => {
  event.preventDefault();
  void run(async () => {
    if (!state?.progress.round || state.pending) return;
    context = await api.game.getContext();
    if (!allowed()) throw new Error("喵币权限不可用，请核对授权后重新打开。");
    const round = state.progress.round;
    const answer = Number(ui.answer.value);
    if (!ui.answer.value.trim() || !Number.isInteger(answer)) { ui.status.textContent = "请输入整数答案。"; return; }
    if (answer !== round.left + round.right) { ui.status.textContent = "再算一算，不会重复扣入场费。"; return; }
    await prepare("reward", {...state.progress, wins: state.progress.wins + 1, round: null}, round.id);
    ui.answer.value = "";
  });
});
ui.retry.addEventListener("click", () => run(async () => {
  await load();
  if (state.pending) await settle();
  else ui.status.textContent = "已重新载入进度。";
}));
ui.close.addEventListener("click", () => run(() => api.game.close()));
async function boot() {
  try {
    if (!api) throw new Error("请从颜喵安装并打开；浏览器模拟宿主不开放真实钱包。");
    await api.game.ready();
    context = await api.game.getContext();
    await load();
    try { ui.balance.textContent = `余额：${(await api.wallet.getBalance()).coins} 喵币`; }
    catch { ui.balance.textContent = "余额暂不可用，以实际钱包结算为准。"; }
    if (state.pending) await settle();
    else ui.status.textContent = allowed() ? "可以开始新局，或继续已保存的题目。" : "此示例需要喵币权限；请允许后重新打开。";
  } catch (error) { ui.status.textContent = `${message(error)}${state?.pending ? "。待结算请求已保留，可重新核对；不要清空存档。" : ""}`; }
  finally { busy = false; render(); }
}
void boot();
