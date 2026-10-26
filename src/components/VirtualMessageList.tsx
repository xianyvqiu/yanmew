import { cloneElement, useEffect, useLayoutEffect, useRef, useState, type ReactElement, type RefObject } from "react";

type Row = { id: string; role: string };
const gap = 14, estimate = 200;

/** 保留可见消息及编辑/选择目标，其余只占位；正文和图片离屏后不再挂载。 */
export default function VirtualMessageList<T extends Row>({ items, viewport, pinnedId, onLayoutChange, renderItem }: {
  onLayoutChange?: () => void; items: T[]; viewport: RefObject<HTMLElement>; pinnedId?: string; renderItem: (item: T) => ReactElement;
}) {
  const root = useRef<HTMLOListElement>(null), heights = useRef(new Map<string, number>());
  const anchor = useRef<{id: string; y: number}>(), resizing = useRef(false);
  const [range, setRange] = useState({ start: Math.max(0, items.length - 20), end: items.length });
  const [revision, setRevision] = useState(0), [selected, setSelected] = useState<string[]>([]);
  const offsets: number[] = [0];
  for (const item of items) offsets.push(offsets[offsets.length - 1] + (heights.current.get(item.id) ?? estimate) + gap);
  const current = useRef({ items, offsets, range, onLayoutChange }); current.current = { items, offsets, range, onLayoutChange };
  useEffect(() => {
    const box = viewport.current, list = root.current; if (!box || !list) return;
    let frame = 0, width = box.clientWidth;
    const update = () => {
      const { items, offsets } = current.current;
      if (!resizing.current) {
        const row = [...list.querySelectorAll<HTMLElement>(":scope > [data-message-id]")].find(e => e.getBoundingClientRect().bottom > box.getBoundingClientRect().top);
        if (row) anchor.current = {id: row.dataset.messageId!, y: row.getBoundingClientRect().top};
      }
      const top = box.scrollTop - (list.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop);
      let start = 0, end = items.length;
      while (start < items.length && offsets[start + 1] < top - 800) start++;
      end = start; while (end < items.length && offsets[end] < top + box.clientHeight + 800) end++;
      if (current.current.range.start !== start || current.current.range.end !== end) current.current.onLayoutChange?.();
      setRange(old => old.start === start && old.end === end ? old : { start, end });
    };
    const schedule = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(update); };
    const selection = () => {
      const selection = window.getSelection();
      const ids = new Set<string>();
      for (const node of [selection?.anchorNode, selection?.focusNode, document.activeElement]) {
        const element = node instanceof Element ? node : node?.parentElement;
        const row = element?.closest<HTMLElement>("[data-message-id]");
        if (row && list.contains(row)) ids.add(row.dataset.messageId!);
      }
      if (selection && !selection.isCollapsed && selection.rangeCount) {
        const range = selection.getRangeAt(0);
        list.querySelectorAll<HTMLElement>(":scope > [data-message-id]").forEach(row => {
          if (range.intersectsNode(row)) ids.add(row.dataset.messageId!);
        });
      }
      setSelected(old => old.join() === [...ids].join() ? old : [...ids]);
    };
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => {
      if (width !== box.clientWidth) { width = box.clientWidth; resizing.current = true; current.current.onLayoutChange?.(); setRevision(v => v + 1); }
      schedule();
    });
    observer?.observe(box); box.addEventListener("scroll", schedule);
    document.addEventListener("selectionchange", selection); list.addEventListener("focusin", selection); list.addEventListener("focusout", selection);
    schedule();
    return () => { cancelAnimationFrame(frame); observer?.disconnect(); box.removeEventListener("scroll", schedule); document.removeEventListener("selectionchange", selection); list.removeEventListener("focusin", selection); list.removeEventListener("focusout", selection); };
  }, [viewport]);
  useLayoutEffect(() => {
    const box = viewport.current, list = root.current; if (!box || !list) return;
    const measure = (element: HTMLElement) => {
      const id = element.dataset.messageId!, height = element.getBoundingClientRect().height;
      if (height <= 0 || heights.current.get(id) === height) return;
      const old = heights.current.get(id) ?? estimate;
      const above = element.getBoundingClientRect().bottom < box.getBoundingClientRect().top;
      current.current.onLayoutChange?.();
      heights.current.set(id, height);
      if (above && !resizing.current) box.scrollTop += height - old;
      setRevision(v => v + 1);
    };
    const rows = [...list.querySelectorAll<HTMLElement>(":scope > [data-message-id]")];
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(entries => entries.forEach(e => measure(e.target as HTMLElement)));
    rows.forEach(row => { measure(row); observer?.observe(row); });
    if (resizing.current && anchor.current) {
      const row = rows.find(row => row.dataset.messageId === anchor.current!.id);
      if (row) box.scrollTop += row.getBoundingClientRect().top - anchor.current.y;
    }
    const settled = requestAnimationFrame(() => { resizing.current = false; });
    return () => { observer?.disconnect(); cancelAnimationFrame(settled); };
  }, [items, range, pinnedId, selected, revision, viewport]);
  const visible = new Set<number>();
  if (items.length <= 60) items.forEach((_, i) => visible.add(i));
  else {
    for (let i = range.start; i < Math.min(items.length, range.end); i++) visible.add(i);
    items.forEach((item, i) => { if (item.id === pinnedId || selected.includes(item.id)) visible.add(i); });
  }
  const nodes: ReactElement[] = []; let previous = 0;
  const spacer = (from: number, to: number) => {
    if (to > from) nodes.push(<li key={`gap-${from}-${to}`} aria-hidden="true" style={{ height: offsets[to] - offsets[from], padding: 0, margin: 0, maxWidth: "none", background: "none", border: 0 }} />);
  };
  for (const index of [...visible].sort((a, b) => a - b)) {
    spacer(previous, index); const item = items[index];
    const element = renderItem(item);
    nodes.push(cloneElement(element, { key: item.id, "data-message-id": item.id, style: { ...element.props.style, width: item.role === "user" ? "fit-content" : undefined, marginBottom: gap, marginLeft: item.role === "user" ? "auto" : undefined }, "aria-posinset": index + 1, "aria-setsize": items.length }));
    previous = index + 1;
  }
  spacer(previous, items.length);
  return <ol ref={root} className="ym-message-list" style={{ display: "block", overflowAnchor: "none" }}>{nodes}</ol>;
}
