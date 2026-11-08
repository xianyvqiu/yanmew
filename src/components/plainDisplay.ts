/** 原文区先隔离再交给 Markdown；代码示例中的标签不作为协议。 */
export function plainDisplay(content: string, expression?: string | null, streaming = false) {
  // 图片模式可能一轮多次换图；只隐藏独立首行标记，不要求与最终动作编号相同。
  if (expression && /^@pet-image:\d+$/.test(expression)) {
    const text = content.trimStart(), firstLine = text.split(/\r?\n/, 1)[0];
    if (/^@pet-image:\d+$/.test(firstLine.trimEnd())) content = text.slice(firstLine.length).replace(/^\r?\n(?:[ \t]*\r?\n)*/, "");
    else if (streaming && text && expression.startsWith(text)) content = "";
  }
  let prefix = "YANMEWPLAINTEXT";
  while (content.includes(prefix)) prefix += "X";
  const values = new Map<string, string>();
  let markdown = "", copy = "", i = 0;
  const append = (text: string) => { markdown += text; copy += text; };
  while (i < content.length) {
    // 围栏代码整体保留；未闭合围栏一直到本次流式文本末尾。
    if (i === 0 || content[i - 1] === "\n") {
      const fence = /^( {0,3})(`{3,}|~{3,})[^\n]*(?:\n|$)/.exec(content.slice(i));
      if (fence) {
        const marker = fence[2];
        const rest = content.slice(i + fence[0].length);
        const closing = new RegExp("^ {0,3}" + marker[0] + "{" + marker.length + ",}[ \\t]*(?:\\n|$)", "m").exec(rest);
        const end = closing ? i + fence[0].length + closing.index + closing[0].length : content.length;
        append(content.slice(i, end)); i = end; continue;
      }
      // Markdown 缩进代码行同样不解释协议。
      const indented = /^(?: {4}|\t)[^\n]*(?:\n|$)/.exec(content.slice(i));
      if (indented) { append(indented[0]); i += indented[0].length; continue; }
    }
    if (content[i] === "\\" && i + 1 < content.length) { append(content.slice(i, i + 2)); i += 2; continue; }
    if (content[i] === "`") {
      const run = /^`+/.exec(content.slice(i))![0];
      const runs = /`+/g; runs.lastIndex = i + run.length;
      let match: RegExpExecArray | null, end = -1;
      while ((match = runs.exec(content))) { if (match[0].length === run.length) { end = match.index + run.length; break; } }
      if (end >= 0) { append(content.slice(i, end)); i = end; continue; }
      append(run); i += run.length; continue;
    }
    if (content.startsWith("<plain>", i)) {
      const start = i + 7, closing = content.indexOf("</plain>", start);
      let end = closing < 0 ? content.length : closing;
      // 不让流式传输中的半个结束标签短暂出现在文字里。
      if (closing < 0) for (let n = 7; n > 0; n--) {
        if (content.slice(start).endsWith("</plain>".slice(0, n))) { end -= n; break; }
      }
      const value = content.slice(start, end), key = prefix + values.size;
      values.set(key, value); markdown += "`" + key + "`"; copy += value;
      i = closing < 0 ? content.length : closing + 8; continue;
    }
    const tail = content.slice(i);
    if (tail.length < 7 && "<plain>".startsWith(tail)) break;
    append(content[i]); i++;
  }
  return { markdown, copy, values };
}
