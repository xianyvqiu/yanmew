import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { JSDOM } from "jsdom";

const css = readFileSync(new URL("../../src/windows/PetWindow.css", import.meta.url), "utf8");

for (const dock of ["peek_left", "peek_right", "rest_sit", "rest_lie"]) {
  test(`${dock}隐藏后显示入口仍位于112×44小窗口内`, () => {
    const dom = new JSDOM();
    try {
      const { document } = dom.window;
      const style = document.createElement("style"); style.textContent = css; document.head.append(style);
      const root = document.createElement("div"); root.className = "pet-window";
      root.dataset.dock = dock; root.dataset.hidden = "true";
      const button = document.createElement("button"); button.className = "pet-visibility";
      root.append(button); document.body.append(root);
      const layout = dom.window.getComputedStyle(button);
      assert.equal(layout.top, "6px"); assert.equal(layout.right, "6px");
      assert.equal(layout.bottom, "auto"); assert.equal(layout.left, "auto");
      assert.ok(parseFloat(layout.width) + parseFloat(layout.right) <= 112);
      assert.ok(parseFloat(layout.height) + parseFloat(layout.top) <= 44);
    } finally { dom.window.close(); }
  });
}

test("聊天提示位于按钮下方，按钮文字保持加粗", () => {
  const dom=new JSDOM();
  try {
    const {document}=dom.window;
    const style=document.createElement("style");style.textContent=css;document.head.append(style);
    document.body.innerHTML='<div class="pet-window"><div class="pet-chat-entry"><button class="ym-button pet-chat-toggle"><span class="pet-tooltip">聊天</span></button></div></div>';
    const tooltip=dom.window.getComputedStyle(document.querySelector('.pet-tooltip'));
    assert.equal(tooltip.bottom,'auto');assert.match(tooltip.top,/100%/);
    assert.equal(dom.window.getComputedStyle(document.querySelector('button')).fontWeight,'700');
  } finally {dom.window.close();}
});
