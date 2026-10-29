import "./CardBrowser.css";
export type BrowserControls = {search:string;order:"newest"|"oldest";page:number;pages:number;total:number;loading?:boolean;error?:string;setSearch:(value:string)=>void;setOrder:(value:"newest"|"oldest")=>void;setPage:(value:number)=>void};
type BrowserProps = {browser:BrowserControls;label:string;timeLabel?:string;disabled?:boolean};
export default function CardBrowserControls({browser,label,timeLabel="时间",disabled=false}:BrowserProps) {
  return <div className="ym-browser-controls" aria-label={`${label}浏览`}>
    <label className="ym-field ym-browser-search">搜索{label}<input type="search" disabled={disabled} value={browser.search} placeholder="输入名称或内容" onChange={e=>browser.setSearch(e.target.value)} /></label>
    <label className="ym-field">按{timeLabel}排序<select disabled={disabled} value={browser.order} onChange={e=>browser.setOrder(e.target.value as "newest"|"oldest")}><option value="newest">最新优先</option><option value="oldest">最早优先</option></select></label>
    {browser.search && !browser.loading && !browser.error && browser.total === 0 && <p role="status" className="ym-page-empty">没有匹配的{label}，请调整搜索词。</p>}
  </div>;
}

/** 放在对应列表与空状态之后，保持阅读顺序与键盘操作顺序一致。 */
export function CardBrowserPagination({browser,label,disabled=false}:BrowserProps) {
  return <nav className="ym-browser-pages" aria-label={`${label}分页`}><button className="ym-button" type="button" disabled={disabled||browser.page<=1} onClick={()=>browser.setPage(browser.page-1)}>上一页</button><span>{browser.page} / {browser.pages} 页 · {browser.total} 条</span><button className="ym-button" type="button" disabled={disabled||browser.page>=browser.pages} onClick={()=>browser.setPage(browser.page+1)}>下一页</button></nav>;
}
