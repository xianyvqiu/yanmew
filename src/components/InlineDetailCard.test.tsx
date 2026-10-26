import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import InlineDetailCard from "./InlineDetailCard";

afterEach(cleanup);
it("展开后保留状态、隐藏重复摘要，收起时焦点回到卡片入口", () => {
  const props={title:"旅行来信",summary:"摘要内容",status:<span>已读</span>,detailLabel:"信件详情",onToggle:()=>{}};
  const view=render(<InlineDetailCard {...props} expanded={false}><button>收藏信件</button></InlineDetailCard>);
  expect(screen.queryByRole("region")).toBeNull();
  view.rerender(<InlineDetailCard {...props} expanded><button>收藏信件</button></InlineDetailCard>);
  expect(screen.getByText("摘要内容").hidden).toBe(true);
  expect(screen.getByText("已读").closest("[hidden]")).toBeNull();
  screen.getByRole("button",{name:"收藏信件"}).focus();
  view.rerender(<InlineDetailCard {...props} expanded={false}><button>收藏信件</button></InlineDetailCard>);
  const trigger=screen.getByRole("button",{expanded:false});
  expect(document.activeElement).toBe(trigger);
  expect(screen.queryByRole("region")).toBeNull();
  expect(screen.getByText("摘要内容").hidden).toBe(false);
  fireEvent.click(trigger);
});
