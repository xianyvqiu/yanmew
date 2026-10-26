//! 游走布局协议；现行窗口移动在pet_desktop。旧画布算法只保留为兼容测试。
use crate::error::{AppError, AppResult};
#[cfg(test)]
use crate::pet::RandomSource;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::time::Duration;

#[cfg(test)]
pub const INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const STEP: f64 = 12.0;
#[cfg(test)]
const MOUSE_RANGE: f64 = 80.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Point { pub x: f64, pub y: f64 }
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }
impl Rect {
    fn valid(self) -> bool {
        [self.x,self.y,self.width,self.height].iter().all(|n| n.is_finite() && n.abs() <= 100_000.0)
            && self.width > 0.0 && self.height > 0.0
    }
#[cfg(test)]
    fn translated(self, p: Point) -> Self { Self {x:self.x+p.x,y:self.y+p.y,..self} }
#[cfg(test)]
    fn contains(self, other: Self) -> bool {
        other.x >= self.x && other.y >= self.y && other.x+other.width <= self.x+self.width
            && other.y+other.height <= self.y+self.height
    }
#[cfg(test)]
    fn overlaps(self, other: Self) -> bool {
        self.x < other.x+other.width && self.x+self.width > other.x
            && self.y < other.y+other.height && self.y+self.height > other.y
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Contact {pub bounds:Rect,pub rows:Vec<u32>}
impl Contact {
    pub fn visible_rect(&self)->Option<Rect> {
        let mut left=32;let mut right=0;let mut top=32;let mut bottom=0;
        for (y,row) in self.rows.iter().enumerate(){if *row!=0 {left=left.min(row.trailing_zeros());right=right.max(32-row.leading_zeros());top=top.min(y);bottom=bottom.max(y+1);}}
        if top==32{return None}let b=self.bounds;Some(Rect{x:b.x+f64::from(left)*b.width/32.0,y:b.y+top as f64*b.height/32.0,width:f64::from(right-left)*b.width/32.0,height:(bottom-top) as f64*b.height/32.0})
    }
    pub fn aim(&self)->Option<Point> {
        let (x,y)=(0..32).flat_map(|y|(0..32).map(move|x|(x,y))).filter(|(x,y)|self.rows.get(*y).is_some_and(|r|r&(1u32<<x)!=0)).min_by_key(|(x,y)|(*x as i32*2-31).pow(2)+(*y as i32*2-31).pow(2))?;
        Some(Point{x:self.bounds.x+(x as f64+0.5)*self.bounds.width/32.0,y:self.bounds.y+(y as f64+0.5)*self.bounds.height/32.0})
    }
    pub fn contains(&self,x:f64,y:f64)->bool {
        let b=self.bounds;
        if x<b.x||y<b.y||x>=b.x+b.width||y>=b.y+b.height{return false}
        let col=((x-b.x)/b.width*32.0).floor() as usize;
        let row=((y-b.y)/b.height*32.0).floor() as usize;
        self.rows.get(row).is_some_and(|bits|bits&(1u32<<col)!=0)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Layout {
    pub persona_id: String, pub session_id: String, pub context_revision: u64,
    pub layout_revision: u64, pub area: Rect, pub body: Rect, pub offset: Point, pub paused: bool,
    #[serde(default)] pub anchor: Option<Rect>,
    #[serde(default)] pub free_y: Option<f64>,
    #[serde(default)] pub contact: Option<Contact>,
}
impl Layout {
    pub fn validate(&self) -> AppResult<()> {
        if self.free_y.is_some_and(|y|!y.is_finite()||y.abs()>100_000.0) || self.contact.as_ref().is_some_and(|c|!c.bounds.valid()||c.rows.len()!=32) || !self.area.valid() || !self.body.valid() || self.anchor.is_some_and(|r|!r.valid()) || !self.offset.x.is_finite() || !self.offset.y.is_finite()
            || self.offset.x.abs() > 100_000.0 || self.offset.y.abs() > 100_000.0 {
            return Err(AppError::validation("layout", "游走画布坐标或尺寸无效"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all="camelCase")]
#[cfg(test)]
pub struct Target {
    pub persona_id: String, pub session_id: String, pub context_revision: u64,
    pub layout_revision: u64, pub offset: Point,
}
#[derive(Default)]
#[cfg(test)]
pub struct Controller { layout: Option<Layout>, last_check: Option<Duration> }
#[cfg(test)]
impl Controller {
    pub fn update(&mut self, layout: Layout) -> AppResult<()> {
        layout.validate()?;
        if self.layout.as_ref().is_some_and(|old| old.layout_revision >= layout.layout_revision) { return Ok(()) }
        if self.layout.as_ref().is_none_or(|old| old.persona_id != layout.persona_id
            || old.session_id != layout.session_id || old.context_revision != layout.context_revision) {
            self.last_check = None;
        }
        self.layout = Some(layout);
        Ok(())
    }
    pub fn due(&self, now: Duration) -> bool {
        self.last_check.is_none_or(|last| now.saturating_sub(last) >= INTERVAL)
    }
    pub fn tick(&mut self, view: &super::controller::View, now: Duration, chat_open: bool,
        mouse: Option<Point>, foreground: Option<Rect>, random: &mut dyn RandomSource) -> AppResult<Option<Target>> {
        let Some(layout) = &mut self.layout else { return Ok(None) };
        if !view.can_wander || chat_open || layout.paused || view.persona_id != layout.persona_id
            || view.session_id.as_deref() != Some(layout.session_id.as_str())
            || view.context_revision != layout.context_revision {
            // 暂停期间不积攒移动次数；解除后等待一个完整低频间隔。
            self.last_check = Some(now);
            return Ok(None)
        }
        if self.last_check.is_some_and(|last| now.saturating_sub(last) < INTERVAL) { return Ok(None) }
        self.last_check = Some(now);
        let start = layout.offset;
        let body = layout.body.translated(start);
        if !layout.area.contains(body) { return Ok(None) }
        if foreground.is_some_and(|rect| rect.contains(layout.area)) { return Ok(None) }
        let near_mouse = mouse.filter(|p| p.x >= layout.area.x-MOUSE_RANGE && p.x <= layout.area.x+layout.area.width+MOUSE_RANGE
            && p.y >= layout.area.y-MOUSE_RANGE && p.y <= layout.area.y+layout.area.height+MOUSE_RANGE);
        let toward = if random.below(4)? == 0 {
            near_mouse.map(|p| Point {x:p.x-(body.x+body.width/2.0),y:p.y-(body.y+body.height/2.0)})
        } else { None };
        let directions = [Point{x:-1.0,y:0.0},Point{x:1.0,y:0.0},Point{x:0.0,y:-1.0},Point{x:0.0,y:1.0},
            Point{x:-0.707,y:-0.707},Point{x:0.707,y:-0.707},Point{x:-0.707,y:0.707},Point{x:0.707,y:0.707}];
        let first = random.below(directions.len() as u64)? as usize;
        if first >= directions.len() { return Err(AppError::new("RANDOM_SOURCE_ERROR", "读取游走方向失败")) }
        for n in 0..=directions.len() {
            let direction = if n == 0 {
                toward.unwrap_or(directions[first])
            } else { directions[(first+n-1)%directions.len()] };
            let distance = direction.x.hypot(direction.y);
            if distance < 0.1 { continue }
            let length = if n==0 && toward.is_some() { STEP.min(distance) } else { STEP };
            let next = Point {x:start.x+direction.x/distance*length,y:start.y+direction.y/distance*length};
            let target = layout.body.translated(next);
            // 当前处在前台覆盖内时允许逐步向外退，但不能进入或加深遮挡。
            let blocked = foreground.is_some_and(|r| target.overlaps(r) && !less_overlap(body,target,r));
            if layout.area.contains(target) && !blocked {
                layout.offset = next;
                return Ok(Some(Target {persona_id:layout.persona_id.clone(),session_id:layout.session_id.clone(),
                    context_revision:layout.context_revision,layout_revision:layout.layout_revision,offset:next}))
            }
        }
        Ok(None)
    }
}
#[cfg(test)]
fn less_overlap(before: Rect, after: Rect, obstacle: Rect) -> bool {
    let area = |r: Rect| ((r.x+r.width).min(obstacle.x+obstacle.width)-r.x.max(obstacle.x)).max(0.0)
        * ((r.y+r.height).min(obstacle.y+obstacle.height)-r.y.max(obstacle.y)).max(0.0);
    area(after) + 0.01 < area(before)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed(u64);
    impl RandomSource for Fixed { fn below(&mut self, upper:u64)->AppResult<u64> { Ok(self.0%upper) } }
    fn layout() -> Layout { Layout {persona_id:"1".into(),session_id:"1".into(),context_revision:0,
        layout_revision:1,area:Rect{x:4.0,y:44.0,width:312.0,height:192.0},
        body:Rect{x:100.0,y:120.0,width:120.0,height:80.0},offset:Point::default(),paused:false,anchor:None,free_y:None,contact:None} }
    fn view() -> super::super::controller::View {
        super::super::controller::View {persona_id:"1".into(),session_id:Some("1".into()),context_revision:0,revision:1,
            activity:super::super::controller::Activity::Normal,expression:"(・ω・)".into(),intimacy:Some(0),
            sleep_until:None,error:None,can_interact:true,can_wander:true}
    }
    #[test]
    fn low_frequency_motion_keeps_entire_footprint_inside_canvas() {
        let mut c=Controller::default();c.update(layout()).unwrap();
        for seconds in 0..300 {
            let target=c.tick(&view(),Duration::from_secs(seconds),false,None,None,&mut Fixed(1)).unwrap();
            if seconds%5!=0 { assert!(target.is_none()); }
            if let Some(target)=target { assert!(layout().area.contains(layout().body.translated(target.offset))); }
        }
    }
    #[test]
    fn foreground_full_coverage_stops_but_free_position_is_used() {
        let mut c=Controller::default();c.update(layout()).unwrap();
        let full=Rect{x:0.0,y:0.0,width:500.0,height:500.0};
        assert!(c.tick(&view(),Duration::ZERO,false,None,Some(full),&mut Fixed(1)).unwrap().is_none());
        let r=Rect{x:220.0,y:120.0,width:50.0,height:80.0};
        let moved=c.tick(&view(),Duration::from_secs(5),false,None,Some(r),&mut Fixed(1)).unwrap().unwrap();
        assert!(!layout().body.translated(moved.offset).overlaps(r));
    }
    #[test]
    fn pause_chat_and_changed_context_never_move_or_accumulate_checks() {
        for reason in 0..4 {
            let mut c=Controller::default();let mut l=layout();l.paused=reason==0;c.update(l).unwrap();
            let mut v=view();if reason==1 {v.can_wander=false;}if reason==2 {v.context_revision=1;}
            assert!(c.tick(&v,Duration::from_secs(100),reason==3,None,None,&mut Fixed(1)).unwrap().is_none());
            c.update(Layout{layout_revision:2,..layout()}).unwrap();
            assert!(c.tick(&view(),Duration::from_secs(101),false,None,None,&mut Fixed(1)).unwrap().is_none());
            assert!(c.tick(&view(),Duration::from_secs(105),false,None,None,&mut Fixed(1)).unwrap().is_some());
        }
    }
    #[test]
    fn nearby_mouse_attracts_once_and_far_mouse_does_not() {
        let mut c=Controller::default();c.update(layout()).unwrap();
        let target=c.tick(&view(),Duration::ZERO,false,Some(Point{x:260.0,y:160.0}),None,&mut Fixed(0)).unwrap().unwrap();
        assert!(target.offset.x>0.0);assert_eq!(target.offset.y,0.0);
        let mut c=Controller::default();c.update(layout()).unwrap();
        let target=c.tick(&view(),Duration::ZERO,false,Some(Point{x:1000.0,y:160.0}),None,&mut Fixed(0)).unwrap().unwrap();
        assert!(target.offset.x<0.0);
    }
    #[test]
    fn invalid_or_old_layout_is_not_applied_and_no_space_stops() {
        let mut c=Controller::default();c.update(layout()).unwrap();
        let mut invalid=layout();invalid.body.x=f64::NAN;assert!(c.update(invalid).is_err());
        c.update(Layout{paused:true,..layout()}).unwrap(); // 同一版不能覆盖已接受布局。
        assert!(c.tick(&view(),Duration::ZERO,false,None,None,&mut Fixed(1)).unwrap().is_some());
        let mut tiny=layout();tiny.layout_revision=2;tiny.area=tiny.body;c.update(tiny).unwrap();
        assert!(c.tick(&view(),Duration::from_secs(5),false,None,None,&mut Fixed(1)).unwrap().is_none());
    }
}
