import { useEffect, useRef, useState } from "react";
export default function ValueChange({ value, unit }: {
    value: string | null | undefined;
    unit: string;
}) {
    const previous = useRef<string | null>(null), [change, setChange] = useState<{
        key: number;
        text: string;
    } | null>(null);
    useEffect(() => { if (value == null)
        return; const before = previous.current; previous.current = value; if (before == null || before === value)
        return; try {
        const delta = BigInt(value) - BigInt(before);
        if (delta !== 0n)
            setChange({ key: Date.now(), text: `${delta > 0 ? "+" : ""}${delta} ${unit}` });
    }
    catch { /* 未加载或非数值不生成效果。 */ } }, [value, unit]);
    return change ? <span key={change.key} className="ym-value-change" onAnimationEnd={() => setChange(null)} aria-live="polite">{change.text}</span> : null;
}
