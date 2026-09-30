use super::*;

fn setup() -> (Runtime, Arc<RequestLease>, NotebookTools) {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    let handler = NotebookTools::new(&runtime, &request, 1, 1, 0).unwrap();
    (runtime, request, handler)
}
fn call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall { index: 0, id: id.into(), name: name.into(), arguments }
}
fn append(id: &str, content: &str) -> ToolCall {
    call(id, UPDATE_TOOL, json!({"operation":"append","section_index":null,"content":content}))
}

#[test]
fn notebook_tools_preserve_over_limit_content_and_deduplicate_writes() {
    let (runtime, _request, mut handler) = setup();
    runtime.read(|store, _| {
        crate::config::save(&mut store.connection, [("notebook_max_chars".into(), "2".into())].into_iter().collect())?;
        Ok(())
    }).unwrap();
    assert_eq!(handler.execute(&append("early", "内容")).unwrap_err().code, "NOTEBOOK_READ_REQUIRED");
    let initial = handler.execute(&call("read-1", READ_TOOL, json!({}))).unwrap();
    assert_eq!(initial.current_chars, 0);
    assert_eq!(initial.remaining_chars, 2);
    let saved = handler.execute(&append("write-1", "中文🙂内容")).unwrap();
    assert_eq!(saved.current_chars, 5);
    assert!(saved.over_limit);
    assert_eq!(saved.remaining_chars, 0);
    assert!(handler.execute(&append("write-1", "中文🙂内容")).unwrap().already_applied);
    assert_eq!(handler.execute(&append("write-1", "别的内容")).unwrap_err().code, "TOOL_CALL_CONFLICT");
    runtime.read(|store, _| {
        assert_eq!(notebook::read(&store.connection, 1)?.unwrap().content, "中文🙂内容");
        Ok(())
    }).unwrap();
    let json = serde_json::to_value(saved).unwrap();
    assert_eq!(json["sections"][0]["section_index"], 1);
    assert_eq!(json["limit_chars"], 2);
}

#[test]
fn notebook_tools_reject_stale_content_until_read_again() {
    let (runtime, _request, mut handler) = setup();
    handler.execute(&call("r1", READ_TOOL, json!({}))).unwrap();
    handler.execute(&append("w1", "旧段落")).unwrap();
    runtime.read(|store, _| {
        let old = notebook::read(&store.connection, 1)?;
        notebook::save(&mut store.connection, 1, &old, "用户新编辑", runtime.clock.sample()?.utc)?;
        Ok(())
    }).unwrap();
    assert_eq!(handler.execute(&append("w2", "追加")).unwrap_err().code, "NOTEBOOK_CONFLICT");
    handler.execute(&call("r2", READ_TOOL, json!({}))).unwrap();
    let result = handler.execute(&call("replace", UPDATE_TOOL, json!({"operation":"replace","section_index":1,"content":"重新读取后修改"}))).unwrap();
    assert_eq!(result.content, "重新读取后修改");
    let result = handler.execute(&call("delete", UPDATE_TOOL, json!({"operation":"delete","section_index":1,"content":null}))).unwrap();
    assert!(result.content.is_empty());
}

#[test]
fn notebook_tools_reject_scope_injection_invalid_arguments_and_cancelled_requests() {
    let (runtime, request, mut handler) = setup();
    assert!(handler.execute(&call("bad-read", READ_TOOL, json!({"session_id":2}))).is_err());
    handler.execute(&call("r", READ_TOOL, json!({}))).unwrap();
    for arguments in [
        json!({"operation":"append","section_index":null,"content":"x","session_id":2}),
        json!({"operation":"append","content":"x"}),
        json!({"operation":"replace","section_index":0,"content":"x"}),
        json!({"operation":"delete","section_index":1,"content":"x"}),
    ] { assert!(handler.execute(&call("invalid", UPDATE_TOOL, arguments)).is_err()); }
    runtime.read(|store, _| { assert!(notebook::read(&store.connection, 1)?.is_none()); Ok(()) }).unwrap();
    request.cancel.cancel();
    assert_eq!(handler.execute(&append("cancelled", "不得写入")).unwrap_err().code, "CANCELLED");
}

#[test]
fn notebook_tools_reject_context_changes_and_publish_closed_schemas() {
    let (runtime, _request, mut handler) = setup();
    handler.execute(&call("r", READ_TOOL, json!({}))).unwrap();
    runtime.read(|store, _| { store.context_revision += 1; Ok(()) }).unwrap();
    assert_eq!(handler.execute(&append("w", "不得写入")).unwrap_err().code, "CONTEXT_CHANGED");
    for definition in definitions() {
        assert_eq!(definition["function"]["parameters"]["additionalProperties"], false);
        assert!(definition["function"]["parameters"]["properties"].get("session_id").is_none());
    }
}
