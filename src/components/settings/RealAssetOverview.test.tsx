import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import type {RealOverview,RealAsset} from "../../realMoneyApi";
import RealAssetOverview from "./RealAssetOverview";
afterEach(cleanup);
const asset=(id:string,balance:string,archived=false):RealAsset=>({id,name:`资金池${id}`,balanceCents:balance,note:"",isDefault:id==="1",archived,revision:1,createdAt:"a",updatedAt:"a"});
it("净总资产保留负数，圆环只按正余额占比",()=>{
 const overview={assets:[asset("1","10000"),asset("2","30000"),asset("3","-5000"),asset("4","0"),asset("5","90000",true)],totalAssetsCents:"35000"}as RealOverview;
 render(<RealAssetOverview overview={overview}/>);expect(screen.getByText("¥350.00")).toBeTruthy();expect(screen.getByText("25.0%")).toBeTruthy();expect(screen.getByText("75.0%")).toBeTruthy();expect(screen.queryByText("资金池3")).toBeNull();expect(screen.queryByText("资金池5")).toBeNull();expect(screen.getByText(/负余额已计入总资产/)).toBeTruthy();
});
it("全部为零或负余额时不绘制无效占比",()=>{
 render(<RealAssetOverview overview={{assets:[asset("1","0"),asset("2","-1")],totalAssetsCents:"-1"}as RealOverview}/>);
 expect(screen.getByText("暂无正余额")).toBeTruthy();expect(document.querySelectorAll(".real-asset-ring")).toHaveLength(0);expect(document.querySelector("svg")?.outerHTML).not.toContain("NaN");
});
