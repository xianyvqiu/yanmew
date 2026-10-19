import { expect, it } from "vitest";
import { plainDisplay } from "./plainDisplay";
it("复制只移除原文区包装，保留 Markdown 和代码示例", () => {
  const source = "**正文** <plain>**字面**</plain> `<plain>示例</plain>`";
  expect(plainDisplay(source).copy).toBe("**正文** **字面** `<plain>示例</plain>`");
});
it("相邻原文区与未闭合原文区均保留内容", () => {
  expect(plainDisplay("<plain>A</plain><plain>B</plain>").copy).toBe("AB");
  expect(plainDisplay("<plain>**未结束").copy).toBe("**未结束");
});
