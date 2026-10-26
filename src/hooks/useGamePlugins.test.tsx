import {act,cleanup,renderHook,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useGamePlugins} from "./useGamePlugins";
const api=vi.hoisted(()=>({list:vi.fn(),listen:vi.fn(),wallet:vi.fn(),listenWallet:vi.fn()}));
vi.mock("../api",()=>({gamePluginsApi:{list:api.list,listen:api.listen},walletApi:{get:api.wallet,listen:api.listenWallet},errorMessage:(e:Error)=>e.message}));
beforeEach(()=>{vi.resetAllMocks();api.list.mockResolvedValue([]);api.wallet.mockResolvedValue({coins:"10",updatedAt:""});api.listen.mockResolvedValue(()=>{});api.listenWallet.mockResolvedValue(()=>{});});
afterEach(cleanup);
it("插件和钱包订阅各自在建立后补查，弥补初次读取间隙",async()=>{
 let pluginReady!:(stop:()=>void)=>void,walletReady!:(stop:()=>void)=>void;
 api.listen.mockImplementation(()=>new Promise(resolve=>{pluginReady=resolve;}));api.listenWallet.mockImplementation(()=>new Promise(resolve=>{walletReady=resolve;}));
 const {result}=renderHook(()=>useGamePlugins());await waitFor(()=>expect(result.current.wallet?.coins).toBe("10"));
 api.list.mockResolvedValue([{pluginId:"new-plugin"}]);await act(async()=>pluginReady(()=>{}));
 await waitFor(()=>expect(result.current.plugins[0]?.pluginId).toBe("new-plugin"));
 api.wallet.mockResolvedValue({coins:"20",updatedAt:""});await act(async()=>walletReady(()=>{}));
 await waitFor(()=>expect(result.current.wallet?.coins).toBe("20"));
});
it("页面关闭后迟到的订阅立即释放，不触发查询",async()=>{
 let ready!:(stop:()=>void)=>void;const stop=vi.fn();api.listen.mockImplementation(()=>new Promise(resolve=>{ready=resolve;}));
 const {unmount}=renderHook(()=>useGamePlugins());await waitFor(()=>expect(api.list).toHaveBeenCalledTimes(1));unmount();
 await act(async()=>ready(stop));expect(stop).toHaveBeenCalledTimes(1);expect(api.list).toHaveBeenCalledTimes(1);
});
