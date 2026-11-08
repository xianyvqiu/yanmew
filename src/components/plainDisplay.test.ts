import { expect, it } from "vitest";
import { plainDisplay } from "./plainDisplay";
it("图片元数据匹配的旧前缀从展示与复制移除，流式半截不闪出",()=>{
  expect(plainDisplay("@pet-image:94  \n\n啊……是挺久了。","@pet-image:94").copy).toBe("啊……是挺久了。");
  expect(plainDisplay("@pet-image:88\n\n正文","@pet-image:57").copy).toBe("正文");
  for(const part of ["@","@pet-","@pet-image:","@pet-image:9","@pet-image:94"]){expect(plainDisplay(part,"@pet-image:94",true).markdown).toBe("");}
  expect(plainDisplay("@","@pet-image:94").copy).toBe("@");
});
it.each(["@朋友 你好","正文里的 @pet-image:94","`@pet-image:94`","```text\n@pet-image:94\n```","<plain>@pet-image:94</plain>","@pet-image:94 是代码示例"])("正常正文或代码不按图片动作前缀处理：%s",source=>{
  expect(plainDisplay(source,"@pet-image:94")).toEqual(plainDisplay(source));
});
it("复制只移除原文区包装，保留 Markdown 和代码示例", () => {
  const source = "**正文** <plain>**字面**</plain> `<plain>示例</plain>`";
  expect(plainDisplay(source).copy).toBe("**正文** **字面** `<plain>示例</plain>`");
});
it("相邻原文区与未闭合原文区均保留内容", () => {
  expect(plainDisplay("<plain>A</plain><plain>B</plain>").copy).toBe("AB");
  expect(plainDisplay("<plain>**未结束").copy).toBe("**未结束");
});
