const status = document.querySelector("#status");
const input = document.querySelector("#text");
const save = document.querySelector("#save");
const close = document.querySelector("#close");
try {
  if (!window.yanmew) throw new Error("请在颜喵宿主打开，或在浏览器开发页注入 mock-host.js。");
  await window.yanmew.game.ready();
  const context = await window.yanmew.game.getContext();
  input.value = (await window.yanmew.storage.get("example.text")) ?? "";
  status.textContent = `宿主已连接，当前运行记录：${context.gameRunId}`;
  save.disabled = close.disabled = false;
  save.addEventListener("click", async () => { try { await window.yanmew.storage.set("example.text", input.value); status.textContent = "已保存，重新打开仍可读取。"; } catch (error) { status.textContent = error.message; } });
  close.addEventListener("click", () => window.yanmew.game.close());
} catch (error) { status.textContent = error.message; }
