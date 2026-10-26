use super::*;
fn context() -> Context {
    Context {
        persona_id: "1".into(),
        session_id: "1".into(),
        context_revision: 0,
    }
}
fn fixture() -> (Runtime, Tools) {
    let rt = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let lease = Arc::new(rt.requests.start("chat:1".into(), 1).unwrap());
    let tools = Tools::new(&rt, &lease, 1, 1, 0);
    (rt, tools)
}
fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        index: 0,
        id: id.into(),
        name: name.into(),
        arguments: args,
    }
}
fn shared(rt: &Runtime) {
    rt.read(|s,_|{s.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'另一人设','原文','原文')",[])?;super::super::save(&mut s.connection,Input{id:None,revision:None,title:"共享规则".into(),summary:"简介".into(),body:"共享原文".into()})?;bind(&s.connection,1,1,true)?;bind(&s.connection,1,2,true)}).unwrap();
}
fn read(tools: &mut Tools) {
    tools
        .execute(&call("read", "get_skill", json!({"skill_id":"1"})))
        .unwrap();
}
fn update() -> Value {
    json!({"skill_id":"1","title":"我的规则","summary":"新简介","body":"只修改我自己的规则"})
}
#[test]
fn create_is_idempotent_and_rejects_other_persona_arguments() {
    let (rt, mut tools) = fixture();
    let create = call(
        "new",
        "create_skill",
        json!({"title":"表达","summary":"简洁","body":"先说结论"}),
    );
    let first = tools.execute(&create).unwrap();
    assert_eq!(first["record"]["bound_to_me"], true);
    assert_eq!(tools.execute(&create).unwrap()["already_applied"], true);
    assert_eq!(
        rt.read(|s, _| Ok(super::super::list(&s.connection)?.len()))
            .unwrap(),
        1
    );
    assert_eq!(
        tools
            .execute(&call(
                "forged",
                "set_skill_binding",
                json!({"skill_id":"1","enabled":false,"persona_id":"2"})
            ))
            .unwrap_err()
            .code,
        "VALIDATION_ERROR"
    );
    assert!(tools
        .execute(&call(
            "bad",
            "create_skill",
            json!({"title":"x","summary":"","body":" "})
        ))
        .is_err());
}
#[test]
fn shared_update_copies_only_own_binding_and_unbind_never_deletes_shared() {
    let (rt, mut tools) = fixture();
    shared(&rt);
    read(&mut tools);
    let result = tools
        .execute(&call("edit", "update_skill", update()))
        .unwrap();
    assert_eq!(result["copied_from"], "1");
    let target = result["record"]["id"].as_str().unwrap();
    rt.read(|s, _| {
        let old = get(&s.connection, 1)?;
        assert_eq!(old.body, "共享原文");
        assert!(!own(&old, 1));
        assert!(own(&old, 2));
        let new = get(&s.connection, parse_id(target)?)?;
        assert!(own(&new, 1));
        assert!(!own(&new, 2));
        Ok(())
    })
    .unwrap();
    tools
        .execute(&call(
            "unbind",
            "set_skill_binding",
            json!({"skill_id":target,"enabled":false}),
        ))
        .unwrap();
    assert_eq!(
        tools
            .execute(&call("denied", "update_skill", update()))
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        rt.read(|s, _| Ok(get(&s.connection, 1)?.personas.len()))
            .unwrap(),
        1
    );
}
#[test]
fn shared_preview_requires_human_confirmation_and_preserves_bindings() {
    let (rt, mut tools) = fixture();
    shared(&rt);
    read(&mut tools);
    let staged = tools
        .execute(&call("preview", "preview_shared_skill_update", update()))
        .unwrap();
    let id = staged["preview_id"].as_str().unwrap();
    assert_eq!(
        rt.read(|s, _| Ok(get(&s.connection, 1)?.body)).unwrap(),
        "共享原文"
    );
    assert_eq!(pending(&rt, context()).unwrap().len(), 1);
    resolve(&rt, context(), id, true).unwrap();
    let saved = rt.read(|s, _| get(&s.connection, 1)).unwrap();
    assert_eq!(saved.body, "只修改我自己的规则");
    assert_eq!(saved.personas.len(), 2);
    assert!(pending(&rt, context()).unwrap().is_empty());
    assert!(resolve(&rt, context(), id, true).is_err());
    assert!(!definitions()
        .iter()
        .any(|d| d["function"]["name"] == "resolve_skill_preview"));
}
#[test]
fn changed_binding_or_context_rejects_shared_confirmation_and_cancel_is_safe() {
    let (rt, mut tools) = fixture();
    shared(&rt);
    read(&mut tools);
    let staged = tools
        .execute(&call("preview", "preview_shared_skill_update", update()))
        .unwrap();
    let id = staged["preview_id"].as_str().unwrap();
    rt.read(|s, _| bind(&s.connection, 1, 2, false)).unwrap();
    assert_eq!(
        resolve(&rt, context(), id, true).unwrap_err().code,
        "SKILL_CONFLICT"
    );
    assert_eq!(
        rt.read(|s, _| Ok(get(&s.connection, 1)?.body)).unwrap(),
        "共享原文"
    );
    resolve(&rt, context(), id, false).unwrap();
    assert!(pending(&rt, context()).unwrap().is_empty());
    rt.read(|s, _| {
        s.context_revision += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        tools
            .execute(&call("stale", "list_skills", json!({})))
            .unwrap_err()
            .code,
        "CONTEXT_CHANGED"
    );
}
#[test]
fn proactive_lease_cannot_execute_skill_tools() {
    let rt = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let lease = Arc::new(rt.requests.start("proactive:1".into(), 1).unwrap());
    let mut tools = Tools::new(&rt, &lease, 1, 1, 0);
    assert!(tools
        .execute(&call(
            "no",
            "create_skill",
            json!({"title":"禁止","summary":"","body":"未授权主动创建"})
        ))
        .is_err());
    assert!(rt
        .read(|s, _| super::super::list(&s.connection))
        .unwrap()
        .is_empty());
}

#[test]
fn background_reads_only_bound_metadata_and_selected_body_and_rejects_writes() {
    let (rt,_)=fixture();
    rt.read(|s,_| {
        for (title,body,bound) in [("旅行语气","需要时读取的正文",true),("烹饪规则","不相关正文",true),("未绑定私有项","不应泄露",false)] {
            super::super::save(&mut s.connection,Input{id:None,revision:None,title:title.into(),summary:"简介".into(),body:body.into()})?;
            if bound {bind(&s.connection,s.connection.last_insert_rowid(),1,true)?;}
        }
        let listing=read_bound(&s.connection,1,&call("list","list_skills",json!({"query":"旅行","limit":1})))?;
        assert_eq!(listing["records"].as_array().unwrap().len(),1);assert!(listing["records"][0].get("body").is_none());
        assert!(!listing.to_string().contains("需要时读取的正文"));
        assert_eq!(read_bound(&s.connection,1,&call("read","get_skill",json!({"skill_id":"1"})))?["record"]["body"],"需要时读取的正文");
        assert_eq!(read_bound(&s.connection,1,&call("other","get_skill",json!({"skill_id":"3"}))).unwrap_err().code,"PERMISSION_DENIED");
        assert!(read_bound(&s.connection,1,&call("library","list_skills",json!({"scope":"library"}))).is_err());
        assert!(read_bound(&s.connection,1,&call("write","create_skill",json!({"title":"禁止写入","summary":"","body":"禁止"}))).is_err());
        let first=read_bound(&s.connection,1,&call("page1","list_skills",json!({"limit":1})))?;
        let second=read_bound(&s.connection,1,&call("page2","list_skills",json!({"limit":1,"before_id":first["next_before_id"]})))?;
        assert_ne!(first["records"][0]["id"],second["records"][0]["id"]);
        assert_eq!(super::super::list(&s.connection)?.len(),3);Ok(())
    }).unwrap();
    let names=read_definitions().iter().map(|v|v["function"]["name"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(names,vec!["list_skills","get_skill"]);
}
