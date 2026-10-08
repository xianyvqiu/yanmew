export type PersonaPromptChange = {
  kind: "equal" | "added" | "removed";
  text: string;
};

// 换行格式与行尾空白不影响含义，其他空白仍作为设定的一部分保留。
export function normalizePersonaPrompt(text: string): string {
  return text.replace(/\r\n?/g, "\n").split("\n").map(line => line.replace(/[\t ]+$/g, "")).join("\n");
}

type Level = 0 | 1 | 2;
type Budget = { remaining: number };
const comparisonBudget = 250_000;

function append(changes: PersonaPromptChange[], kind: PersonaPromptChange["kind"], text: string) {
  if (!text) return;
  const previous = changes[changes.length - 1];
  if (previous?.kind === kind) previous.text += text;
  else changes.push({ kind, text });
}

function tokens(text: string, level: Level): string[] {
  if (level === 0) return text.match(/[^\n]+\n*|\n+/gu) ?? [];
  if (level === 1) return text.match(/[^。！？.!?；;\n]+[。！？.!?；;\n]*|[。！？.!?；;\n]+/gu) ?? [];
  // 按 Unicode 码点比较，避免把颜文字或 emoji 的代理对拆开。
  return Array.from(text);
}

function compare(before: string, after: string, level: Level, budget: Budget): PersonaPromptChange[] {
  if (before === after) return before ? [{ kind: "equal", text: before }] : [];
  if (!before) return [{ kind: "added", text: after }];
  if (!after) return [{ kind: "removed", text: before }];
  const left = tokens(before, level);
  const right = tokens(after, level);
  let start = 0;
  while (start < left.length && start < right.length && left[start] === right[start]) start++;
  let end = 0;
  while (end < left.length - start && end < right.length - start
    && left[left.length - end - 1] === right[right.length - end - 1]) end++;
  const a = left.slice(start, left.length - end);
  const b = right.slice(start, right.length - end);
  const changes: PersonaPromptChange[] = [];
  append(changes, "equal", left.slice(0, start).join(""));

  const refine = (oldText: string, newText: string) => {
    if (level < 2 && oldText && newText) {
      for (const change of compare(oldText, newText, (level + 1) as Level, budget)) {
        append(changes, change.kind, change.text);
      }
    } else {
      append(changes, "removed", oldText);
      append(changes, "added", newText);
    }
  };

  const cells = (a.length + 1) * (b.length + 1);
  if (!a.length || !b.length || cells > budget.remaining) {
    // 整次比较共用运算上限，超长变更逐层细化后按完整片段展示，不截断原文。
    refine(a.join(""), b.join(""));
  } else {
    budget.remaining -= cells;
    const width = b.length + 1;
    const lengths = new Uint32Array(cells);
    for (let i = a.length - 1; i >= 0; i--) {
      for (let j = b.length - 1; j >= 0; j--) {
        lengths[i * width + j] = a[i] === b[j]
          ? lengths[(i + 1) * width + j + 1] + 1
          : Math.max(lengths[(i + 1) * width + j], lengths[i * width + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    let oldParts: string[] = [];
    let newParts: string[] = [];
    const flush = () => {
      refine(oldParts.join(""), newParts.join(""));
      oldParts = [];
      newParts = [];
    };
    while (i < a.length || j < b.length) {
      if (i < a.length && j < b.length && a[i] === b[j]) {
        flush();
        append(changes, "equal", a[i]);
        i++; j++;
      } else if (i < a.length && (j === b.length || lengths[(i + 1) * width + j] >= lengths[i * width + j + 1])) {
        oldParts.push(a[i++]);
      } else {
        newParts.push(b[j++]);
      }
    }
    flush();
  }
  append(changes, "equal", end ? left.slice(left.length - end).join("") : "");
  return changes;
}

/** 纯本地累计差异，不调用 AI、不修改传入文本或保存状态。 */
export function personaPromptDiff(initialPrompt: string, currentPrompt: string): PersonaPromptChange[] {
  return compare(normalizePersonaPrompt(initialPrompt), normalizePersonaPrompt(currentPrompt), 0,
    { remaining: comparisonBudget });
}
