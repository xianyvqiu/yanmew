import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import ChatUsageNote from "./ChatUsageNote";
afterEach(cleanup);
it("未返回用量时不编造缓存命中数",()=>{render(<ChatUsageNote usage={{requests:1,inputRequests:0,cachedRequests:0,inputTokens:0,cachedTokens:0}}/>);expect(screen.getByText(/输入 未提供/).textContent).toContain("缓存命中 未提供");});
it("服务商明确报告零命中时显示0",()=>{render(<ChatUsageNote usage={{requests:1,inputRequests:1,cachedRequests:1,inputTokens:100,cachedTokens:0}}/>);expect(screen.getByText(/缓存命中 0 tokens/)).toBeTruthy();});
it("部分请求缺失时标注统计不完整",()=>{render(<ChatUsageNote usage={{requests:3,inputRequests:2,cachedRequests:1,inputTokens:300,cachedTokens:70}}/>);expect(screen.getByText(/部分请求未提供完整统计/).textContent).toContain("缓存命中 70 tokens");});
it("等待第一份响应期间不增加空统计",()=>{const {container}=render(<ChatUsageNote/>);expect(container.textContent).toBe("");});
