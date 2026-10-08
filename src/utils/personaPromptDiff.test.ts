import { describe, expect, it } from "vitest";
import { normalizePersonaPrompt, personaPromptDiff } from "./personaPromptDiff";

describe("personaPromptDiff", () => {
  it.each([
    ["一致", "冷淡，但关心对方。", "冷淡，但关心对方。"],
    ["纯新增", "", "慢热。"],
    ["纯删除", "慢热。", ""],
    ["句内替换", "说话冷淡，保持耐心。", "说话克制，保持耐心。"],
    ["多段增删", "身份：电子小猫。\n慢热。\n喜欢安静。", "身份：电子小猫。\n温和但慢热。\n喜欢读书。"],
    ["重复句子", "慢热。慢热。耐心。", "慢热。耐心。耐心。"],
    ["换行和行尾空白", "慢热。 \r\n耐心。\t", "慢热。\n耐心。"],
    ["Unicode 与颜文字", "( • ω • ) 🐱 很耐心。", "( • ω • ) 🐈 很耐心。"],
    ["空文本", "", ""],
  ])("%s：两侧都能完整还原，不漏字、不串顺序", (_, before, after) => {
    const changes = personaPromptDiff(before, after);
    expect(changes.filter(change => change.kind !== "added").map(change => change.text).join(""))
      .toBe(normalizePersonaPrompt(before));
    expect(changes.filter(change => change.kind !== "removed").map(change => change.text).join(""))
      .toBe(normalizePersonaPrompt(after));
  });

  it("中文局部替换只标记改动字，其他内容保持普通正文", () => {
    expect(personaPromptDiff("说话冷淡，保持耐心。", "说话克制，保持耐心。")).toEqual([
      { kind: "equal", text: "说话" },
      { kind: "removed", text: "冷淡" },
      { kind: "added", text: "克制" },
      { kind: "equal", text: "，保持耐心。" },
    ]);
  });

  it("CRLF 和行尾空白不制造差异，内部空格仍有意义", () => {
    expect(personaPromptDiff("慢热。 \r\n耐心。\t", "慢热。\n耐心。").every(change => change.kind === "equal")).toBe(true);
    expect(personaPromptDiff("称呼：小 猫", "称呼：小猫").some(change => change.kind === "removed" && change.text === " ")).toBe(true);
  });

  it("超长文本局部修改不使整篇设定都变成新增/删除", () => {
    const prefix = "保持原身份，慢热但关心对方。\n".repeat(4000);
    const changes = personaPromptDiff(`${prefix}表达冷淡。`, `${prefix}表达克制。`);
    expect(changes.filter(change => change.kind === "removed").map(change => change.text).join("")).toBe("冷淡");
    expect(changes.filter(change => change.kind === "added").map(change => change.text).join("")).toBe("克制");
    expect(changes[0].text).toBe(`${prefix}表达`);
  });

  it("超长完全不同的文本在比较上限内退化为完整片段，不截断内容", () => {
    const before = "甲".repeat(100_000);
    const after = "乙".repeat(100_000);
    expect(personaPromptDiff(before, after)).toEqual([
      { kind: "removed", text: before }, { kind: "added", text: after },
    ]);
  });
});
