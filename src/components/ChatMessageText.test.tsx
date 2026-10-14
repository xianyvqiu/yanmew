import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import ChatMessageText from "./ChatMessageText";
const open = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock("../api", () => ({ webSearchApi: { open }, errorMessage: (e: { message: string }) => e.message }));
afterEach(() => { cleanup(); open.mockClear(); });
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
