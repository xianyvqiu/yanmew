import {useCallback,useEffect,useRef,useState} from "react";
import {cardBrowserApi,errorMessage,type CardKind} from "../api";
import type {CurrentContext} from "../types";

/** 仓储搜索完整集合后返回一页；界面不收集全部正文。 */
export function useRemoteCardBrowser<T>(kind:CardKind,context:CurrentContext) {
  const [query,setQuery]=useState({search:"",order:"newest" as "newest"|"oldest",page:1});
  const [refreshToken,setRefreshToken]=useState(0);
  const key=`${kind}:${context.personaId}:${context.sessionId}:${context.contextRevision}`;
  const queryKey=`${key}:${JSON.stringify(query)}`;
  const [result,setResult]=useState<{key:string;items:T[];total:number;page:number;pages:number}>({key:"",items:[],total:0,page:1,pages:1});
  const [error,setError]=useState("");const [loading,setLoading]=useState(false);const generation=useRef(0);
  const refresh=useCallback(()=>{setRefreshToken(n=>n+1);},[]);
  useEffect(()=>{setQuery({search:"",order:"newest",page:1});},[key]);
  useEffect(()=>{
    const token=++generation.current;setError("");
    if(!context.sessionId){setLoading(false);return;}
    setLoading(true);
    void cardBrowserApi.browse<T>(kind,context,query.search,query.order,query.page).then(page=>{if(token===generation.current)setResult({key:queryKey,...page});}).catch(cause=>{if(token===generation.current)setError(errorMessage(cause));}).finally(()=>{if(token===generation.current)setLoading(false);});
    return()=>{generation.current++;};
  },[queryKey,refreshToken]);
  useEffect(()=>{
    let disposed=false,stop:(()=>void)|undefined;
    void cardBrowserApi.listen(kind,refresh).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});
    window.addEventListener("focus",refresh);return()=>{disposed=true;stop?.();window.removeEventListener("focus",refresh);};
  },[kind,refresh]);
  const current=result.key===queryKey?result:{items:[] as T[],total:0,page:query.page,pages:1};
  return {...current,search:query.search,order:query.order,error,loading,refresh,
    setSearch:(search:string)=>setQuery(q=>({...q,search,page:1})),setOrder:(order:"newest"|"oldest")=>setQuery(q=>({...q,order,page:1})),setPage:(page:number)=>setQuery(q=>({...q,page}))};
}
