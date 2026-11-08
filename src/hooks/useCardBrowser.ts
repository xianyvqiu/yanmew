import { useMemo, useState } from "react";

/** 完整摘要集合的浏览状态；查询后分页，不对当前页做搜索。 */
export function useCardBrowser<T>(items: T[], text: (item:T)=>string, time:(item:T)=>string, id:(item:T)=>string, pageSize=12, priority?:(item:T)=>number) {
  const [search,setSearch]=useState("");
  const [order,setOrder]=useState<"newest"|"oldest">("newest");
  const [requestedPage,setPage]=useState(1);
  const matching=useMemo(()=>{
    const words=search.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    return items.filter(item=>words.every(word=>text(item).toLocaleLowerCase().includes(word)))
      .sort((a,b)=>{
        const preferred=(priority?.(b)??0)-(priority?.(a)??0);
        if(preferred)return preferred;
        const difference=time(a).localeCompare(time(b))||id(a).localeCompare(id(b),undefined,{numeric:true});
        return order==="newest"?-difference:difference;
      });
  },[items,search,order,text,time,id,priority]);
  const pages=Math.max(1,Math.ceil(matching.length/pageSize));
  const page=Math.min(requestedPage,pages);
  return {search,order,page,pages,total:matching.length,items:matching.slice((page-1)*pageSize,page*pageSize),
    setSearch:(value:string)=>{setSearch(value);setPage(1);},setOrder:(value:"newest"|"oldest")=>{setOrder(value);setPage(1);},setPage};
}
