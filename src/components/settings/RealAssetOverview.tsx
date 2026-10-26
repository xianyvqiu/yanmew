import { moneyText, type RealOverview } from "../../realMoneyApi";
import "./RealAssetOverview.css";

export default function RealAssetOverview({overview}:{overview:RealOverview}) {
    const assets=overview.assets.filter(asset=>!asset.archived);
    const positive=assets.filter(asset=>BigInt(asset.balanceCents)>0n);
    const total=positive.reduce((sum,asset)=>sum+BigInt(asset.balanceCents),0n);
    const negative=assets.reduce((sum,asset)=>sum+(BigInt(asset.balanceCents)<0n?BigInt(asset.balanceCents):0n),0n);
    let offset=0;
    return <div className="real-asset-overview" aria-label="资产概览">
        <section className="real-asset-total"><span>总资产</span><strong>¥{moneyText(overview.totalAssetsCents)}</strong><p>{assets.length} 个资金池的余额合计</p><dl><div><dt>正余额合计</dt><dd>¥{moneyText(total.toString())}</dd></div><div><dt>负余额合计</dt><dd data-negative={negative<0n}>¥{moneyText(negative.toString())}</dd></div></dl></section>
        <section className="real-asset-distribution"><header><h3>资产分布</h3><span>按资金池正余额</span></header><div className="real-asset-distribution-body"><div className="real-asset-donut"><svg viewBox="0 0 200 200" role="img" aria-label="资产比例圆环图"><circle cx="100" cy="100" r="75" className="real-asset-ring-track"/>{positive.map((asset,index)=>{
            const share=Number(BigInt(asset.balanceCents)*1_000_000n/total)/10_000,at=offset;offset+=share;
            return <circle key={asset.id} cx="100" cy="100" r="75" pathLength="100" strokeDasharray={`${share} ${100-share}`} strokeDashoffset={-at} transform="rotate(-90 100 100)" className="real-asset-ring" data-color={index%6}><title>{asset.name}：¥{moneyText(asset.balanceCents)}，占{share.toFixed(1)}%</title></circle>;
        })}</svg><div><strong>{positive.length}</strong><span>{total>0n?"个正余额资金池":"暂无正余额"}</span></div></div><ul className="real-asset-legend">{positive.map((asset,index)=><li key={asset.id} data-color={index%6}><i aria-hidden="true"/><span>{asset.name}<small>¥{moneyText(asset.balanceCents)}</small></span><strong>{(Number(BigInt(asset.balanceCents)*1000n/total)/10).toFixed(1)}%</strong></li>)}{!positive.length&&<li className="real-asset-empty">为资金池记录余额后，会在这里展示资产比例。</li>}</ul></div>{negative<0n&&<p className="real-asset-negative-note">负余额已计入总资产，不参与正余额圆环占比。</p>}</section>
    </div>;
}
