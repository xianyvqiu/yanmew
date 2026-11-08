import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import ChatMessageText from "./ChatMessageText";
const open = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock("../api", () => ({ webSearchApi: { open }, errorMessage: (e: { message: string }) => e.message }));
afterEach(() => { cleanup(); open.mockClear(); });
it("旧AI回复隐藏匹配动作前缀，用户消息和代码原样保留",()=>{
  const view=render(<ChatMessageText content={"@pet-image:94\n\n你好"} expression="@pet-image:94"/>);
  expect(view.container.textContent).toBe("你好");
  view.rerender(<ChatMessageText content={"@pet-image:94\n\n你好"} expression="@pet-image:94" markdown={false}/>);
  expect(view.container.textContent).toContain("@pet-image:94");
  view.rerender(<ChatMessageText content="`@pet-image:94`" expression="@pet-image:94"/>);
  expect(view.container.querySelector("code")?.textContent).toBe("@pet-image:94");
});
it("来源链接通过宿主打开，颜文字和 HTML 作为普通文字保留", async () => {
  render(<ChatMessageText content={"(・ω・) <script>文字</script> [官方来源](https://example.com/news)"} />);
  expect(screen.getByText(/\(・ω・\)/).textContent).toContain("<script>文字</script>");
  fireEvent.click(screen.getByRole("link", { name: "官方来源" }));
  await waitFor(() => expect(open).toHaveBeenCalledWith("https://example.com/news"));
});
it("带凭据的地址不变成可点击来源", () => {
  render(<ChatMessageText content="https://user:password@example.com/" />);
  expect(screen.queryByRole("link")).toBeNull();
});

it("AI 正文支持标题、强调、引用、列表、代码和 GFM 表格", () => {
  const { container } = render(<ChatMessageText content={[
    "## 一起学习", "", "**重点**和*强调*，还有 `let cat = 1`。", "", "> 引用内容", "",
    "1. 第一步", "2. 第二步", "", "- [x] 已完成", "- [ ] 待完成", "",
    "| 名称 | 数量 |", "| --- | ---: |", "| 小猫 | 2 |",
  ].join("\n")} />);
  expect(screen.getByRole("heading", { name: "一起学习", level: 2 })).toBeTruthy();
  expect(container.querySelector("strong")?.textContent).toBe("重点");
  expect(container.querySelector("em")?.textContent).toBe("强调");
  expect(container.querySelector("blockquote")?.textContent).toContain("引用内容");
  expect(container.querySelector("code")?.textContent).toBe("let cat = 1");
  expect(screen.getAllByRole("listitem")).toHaveLength(4);
  expect(screen.getAllByRole("checkbox").every(box => (box as HTMLInputElement).disabled)).toBe(true);
  expect(screen.getByRole("region", { name: "表格" }).tabIndex).toBe(0);
  expect(screen.getByRole("cell", { name: "小猫" })).toBeTruthy();
});

it("流式未闭合的代码块可以显示，更新完成后没有重复文字", () => {
  const view = render(<ChatMessageText content={'解释\n\n```js\nconst cat = "喵'} />);
  expect(view.container.querySelector("pre code")?.textContent).toBe('const cat = "喵\n');
  view.rerender(<ChatMessageText content={'解释\n\n```js\nconst cat = "喵";\n```\n\n**完成**'} />);
  expect(view.container.querySelectorAll("pre")).toHaveLength(1);
  expect(view.container.querySelector("pre code")?.textContent).toBe('const cat = "喵";\n');
  expect(view.container.querySelector("pre")?.tabIndex).toBe(0);
  expect(view.container.querySelector("strong")?.textContent).toBe("完成");
});

it("用户消息保留 Markdown 标记与换行，仍可点击原有来源链接", async () => {
  const content = "## 原样标题\n**原样强调**\n[来源](https://example.com/)";
  const { container } = render(<ChatMessageText markdown={false} content={content} />);
  expect(container.querySelector("p")?.textContent).toContain("## 原样标题\n**原样强调**");
  expect(container.querySelector("h2, strong")).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "来源" }));
  await waitFor(() => expect(open).toHaveBeenCalledWith("https://example.com/"));
});

it("颜文字与单波浪号保持普通文字，只有双波浪号表示删除线", () => {
  const { container } = render(<ChatMessageText content="(・ω・) (^_^) ~喵~ ~~删除~~" />);
  expect(container.textContent).toContain("(・ω・) (^_^) ~喵~");
  expect(container.querySelectorAll("del")).toHaveLength(1);
  expect(container.querySelector("del")?.textContent).toBe("删除");
});

it("带星号/下划线的常见颜文字不丢符号，普通强调仍正常渲染", () => {
  const faces = "(*^_^*) (*>_<*) _(:з」∠)_ (**^ω^**) （*・ω・*）";
  const { container } = render(<ChatMessageText content={`${faces}\n\n**重点** (*说明*) (**2^3**)`} />);
  expect(container.textContent).toContain(faces);
  expect([...container.querySelectorAll("strong")].map(node => node.textContent)).toEqual(["重点", "2^3"]);
  expect(container.querySelector("em")?.textContent).toBe("说明");
});

it("颜文字保护不改变行内/围栏代码与链接 URL，流式标记闭合后恢复原样", async () => {
  const view = render(<ChatMessageText content={"`(*^_^*)`\n\n```text\n(**^ω^**)\n```\n\n[来源](https://example.com/(*%5E_%5E*))"} />);
  expect([...view.container.querySelectorAll("code")].map(node => node.textContent)).toEqual(["(*^_^*)", "(**^ω^**)\n"]);
  fireEvent.click(screen.getByRole("link", { name: "来源" }));
  await waitFor(() => expect(open).toHaveBeenCalledWith("https://example.com/(*%5E_%5E*)"));
  view.rerender(<ChatMessageText content="你好 (*^_" />);
  view.rerender(<ChatMessageText content="你好 (*^_^*)" />);
  expect(view.container.textContent).toBe("你好 (*^_^*)");
  expect(view.container.querySelector("em")).toBeNull();
});

it("不执行 HTML、不自动加载图片，非 HTTP(S) 或带凭据的链接不可跳转", () => {
  const { container } = render(<ChatMessageText content={[
    '<img src="https://example.com/track" onerror="alert(1)">', "",
    "![示意图](https://example.com/image.png)", "",
    "[脚本](javascript:alert%281%29) [数据](data:text/html,test) [文件](file:///C:/test)",
    "[凭据](https://user:secret@example.com/) [相对路径](/settings) [邮件](mailto:test@example.com)",
  ].join("\n")} />);
  expect(container.querySelector("img, script, iframe")).toBeNull();
  expect(container.textContent).toContain("[图片：示意图]");
  expect(screen.queryByRole("link")).toBeNull();
  expect(open).not.toHaveBeenCalled();
});

it("打开链接失败显示局部提示，不丢失消息正文", async () => {
  open.mockRejectedValueOnce(new Error("打开失败"));
  render(<ChatMessageText content="**正文保留** [来源](https://example.com/news)" />);
  fireEvent.click(screen.getByRole("link", { name: "来源" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("打开失败"));
  expect(screen.getByText("正文保留").tagName).toBe("STRONG");
});


it("通用原文区保留符号、换行和 HTML，不生成代码样式或链接", () => {
  const raw = String.raw`(*^_^*) \(^o^)/ **原样**
# 不渲染标题
<script>原文</script> https://example.com`;
  const view = render(<ChatMessageText content={`**正文** <plain>${raw}</plain> *末尾*`} />);
  expect(view.container.querySelector(".ym-plain-display")?.textContent).toBe(raw);
  expect(view.container.querySelector("strong")?.textContent).toBe("正文");
  expect(view.container.querySelector("em")?.textContent).toBe("末尾");
  expect(view.container.querySelector("code,script,a,h1")).toBeNull();
});

it("原文区流式未闭合及半截标签不闪烁，代码示例不消耗标签", () => {
  const view = render(<ChatMessageText content="前文 <pla" />);
  expect(view.container.textContent).toBe("前文");
  view.rerender(<ChatMessageText content="前文 <plain>**字面**</pla" />);
  expect(view.container.textContent).toBe("前文 **字面**");
  view.rerender(<ChatMessageText content="前文 <plain>**字面**</plain> **完成**" />);
  expect(view.container.textContent).toBe("前文 **字面** 完成");
  view.rerender(<ChatMessageText content={'`<plain>*示例*</plain>`\n\n```text\n<plain>示例</plain>\n```'} />);
  expect([...view.container.querySelectorAll("code")].map(n => n.textContent)).toEqual(["<plain>*示例*</plain>", "<plain>示例</plain>\n"]);
});
