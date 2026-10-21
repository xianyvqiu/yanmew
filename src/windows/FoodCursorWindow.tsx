import { useLayoutEffect } from "react";
import { foodImage } from "../companionApi";
export default function FoodCursorWindow() {
  useLayoutEffect(() => {
    document.documentElement.style.setProperty("background", "transparent", "important");
    document.body.style.setProperty("background", "transparent", "important");
    document.body.style.margin = "0";
  }, []);
  const id = new URLSearchParams(window.location.search).get("food") ?? "";
  return <div className="food-cursor-display" aria-hidden="true"><img src={foodImage(id)} alt="" /><span>点击桌宠投喂 · 右键取消</span></div>;
}
