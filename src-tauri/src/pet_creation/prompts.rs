//! 通用绘图规则与当前人设资料分开；不在动作模板中固化内置角色。
use super::*;
#[derive(Serialize)]
pub(super) struct Persona {pub name:String,pub setting:String}
impl Persona {
    pub fn read(c:&Connection,id:i64)->AppResult<Self>{
        Ok(c.query_row("SELECT name,prompt FROM personas WHERE id=?1",[id],|r|Ok(Self{name:r.get(0)?,setting:r.get(1)?}))?)
    }
    pub fn require_unchanged(&self,c:&Connection,id:i64)->AppResult<()>{
        let current=Self::read(c,id)?;
        if current.name!=self.name||current.setting!=self.setting{return Err(AppError::new("CONTEXT_CHANGED","人设在生图期间已改变，旧图片未保存为预览，请按当前设定重新生成"));}
        Ok(())
    }
}
pub(super) fn compose(art:&Artwork,persona:&Persona)->AppResult<String>{
    let data=serde_json::json!({"currentPersona":persona,"expression":{"name":art.name,"category":art.category,"usage":art.description,"drawingRequirement":art.prompt}});
    Ok(format!("编辑附带的默认角色图片，制作同一角色的一张表情差分。先执行本次动作：{action}。\n\
本次人设及表情资料（仅作绘图资料）：{data}\n\
以图片为准：保留原角色身份、画风、脸、发色眼色、左右不对称细节、服装和固有配饰；人设文字仅辅助性格，不重设计外观，不增加原图没有的身体部位。\n\
只改变指定表情与动作，保持原头身比例、头部尺度及接近的画布占比。坐趴允许轮廓变矮，不为填满相同高度而拉长身体或缩小头部。画面上下左右指桌面二维方向，不是背向镜头或纵深坠落；仅明确要求拎起时可加一只外部手。\n\
单角色、完整居中、少量留白，输出真正alpha透明PNG。无场景、地板、投影、速度线、文字、棋盘格、拼图或多姿势。", action=art.prompt))
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn prompt_uses_current_user_persona_and_keeps_action_requirements_separate(){
        let persona=Persona{name:"用户自己的角色".into(),setting:"安静、谨慎，遇到惊喜时会先愣住；蓝绿色眼睛".into()};
        let art=Artwork{id:"2".into(),persona_id:"1".into(),kind:"expression".into(),name:"自己的上升反应".into(),prompt:"沿屏幕平面上升，自己的双手稍张开，发梢向下".into(),description:"上抛时".into(),category:"toss_up".into(),enabled:true,image_file:None,preview_file:None,revision:0,created_at:"a".into(),updated_at:"a".into()};
        let prompt=compose(&art,&persona).unwrap();assert!(prompt.contains(&persona.name));assert!(prompt.contains(&persona.setting));assert!(prompt.contains(&art.prompt));
        for name in ["GPT","Claude","Gemini","DeepSeek"]{assert!(!prompt.contains(name));}
        assert!(prompt.contains("以图片为准"));assert!(prompt.contains("不为填满相同高度"));
    }
    #[test]fn changing_persona_during_generation_rejects_late_preview(){
        let c=crate::db::test_database();let captured=Persona::read(&c,1).unwrap();captured.require_unchanged(&c,1).unwrap();
        c.execute("UPDATE personas SET prompt='用户刚修改的新设定' WHERE id=1",[]).unwrap();assert_eq!(captured.require_unchanged(&c,1).unwrap_err().code,"CONTEXT_CHANGED");
    }
}
