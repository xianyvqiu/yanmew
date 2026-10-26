import type {ProviderUsage} from "../types";
/** 仅展示服务商已报告的正文请求，不将估算或缺失数据当作账单。 */
export default function ChatUsageNote({usage}:{usage?:ProviderUsage|null}){
 if(!usage||usage.requests===0)return null;
 const partial=(usage.inputRequests>0&&usage.inputRequests<usage.requests)||(usage.cachedRequests>0&&usage.cachedRequests<usage.requests);
 return <span style={{display:"block",marginTop:6}}>服务商已报告用量（本轮正文及工具往返）：
   输入 {usage.inputRequests?`${usage.inputTokens.toLocaleString()} tokens`:"未提供"}；
   缓存命中 {usage.cachedRequests?`${usage.cachedTokens.toLocaleString()} tokens`:"未提供"}。
   {partial&&"部分请求未提供完整统计。"}
 </span>;
}
