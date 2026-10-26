import land from "../../assets/maps/natural-earth-land.json";
export type MapPoint={longitude:number;latitude:number};
const mercator=(latitude:number)=>Math.log(Math.tan(Math.PI/4+Math.max(-85,Math.min(85,latitude))*Math.PI/360))*180/Math.PI;
const latitude=(y:number)=>(2*Math.atan(Math.exp(y*Math.PI/180))-Math.PI/2)*180/Math.PI;
const near=(longitude:number,center:number)=>longitude+Math.round((center-longitude)/360)*360;
export function typhoonMapGeometry(points:MapPoint[],local:MapPoint){
    const all=[...points,local],xs=all.map(p=>near(p.longitude,local.longitude)),ys=all.map(p=>mercator(p.latitude));
    const cx=(Math.min(...xs)+Math.max(...xs))/2,cy=(Math.min(...ys)+Math.max(...ys))/2;
    const scale=Math.max((Math.max(8,Math.max(...xs)-Math.min(...xs))+4)/660,(Math.max(5,Math.max(...ys)-Math.min(...ys))+4)/288);
    const project=(p:MapPoint,wrap=true)=>({x:360+((wrap?near(p.longitude,local.longitude):p.longitude)-cx)/scale,y:164-(mercator(p.latitude)-cy)/scale});
    const west=cx-340*scale,east=cx+340*scale,north=latitude(cy+146*scale),south=latitude(cy-146*scale);
    const ticks=(min:number,max:number)=>{const step=[1,2,5,10,20,30,60].find(s=>(max-min)/s<=7)??90;const values=[];for(let n=Math.ceil(min/step)*step;n<=max;n+=step)values.push(n);return values;};
    const paths=land.rings.flatMap(ring=>{
        const center=(Math.min(...ring.map(p=>p[0]))+Math.max(...ring.map(p=>p[0])))/2;
        const shift=Math.round((local.longitude-center)/360)*360;
        const coordinates=ring.map(p=>({longitude:p[0]+shift,latitude:p[1]}));
        if(Math.max(...coordinates.map(p=>p.longitude))<west||Math.min(...coordinates.map(p=>p.longitude))>east||Math.max(...coordinates.map(p=>p.latitude))<south||Math.min(...coordinates.map(p=>p.latitude))>north)return [];
        return [coordinates.map((p,i)=>{const at=project(p,false);return `${i?"L":"M"}${at.x.toFixed(1)},${at.y.toFixed(1)}`;}).join(" ")+"Z"];
    });
    return {project,paths,west,east,north,south,longitudes:ticks(west,east),latitudes:ticks(south,north)};
}
