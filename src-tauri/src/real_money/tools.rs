use super::*;
use crate::{ai::provider::ToolCall, runtime::Runtime, state::RequestLease};
use std::sync::Arc;
pub const RULES:&str="现实记账使用人民币，与喵币及游戏钱包独立。只有用户明确要求记账且金额性质清楚时才写入；谈价格、咨询、举例或假设不等于记账。只说“花了多少”“记一笔”而性质不明时，先问是否现实资金。按需查询分类和资产；未指定资产可用唯一默认项，回复时说明。不合适的分类可按用户意图新建，修改/删除分类前先查询其ID和revision。时间未指定时用当前本地时间，昨天等相对日期换成真实日期；日期或周期含糊时先问。周期支持每N天/周/月/年，首次日期为anchor，短月取月末；漏期由用户在现实记账界面确认，不代为批量补记。完成后报告标题、金额、收支、分类、资产、日期及是否周期。仅查询当前问题需要的范围。";
pub fn is_write(name: &str) -> bool {
    matches!(
        name,
        "record_real_money" | "save_real_money_category" | "delete_real_money_category"
    )
}
pub fn definitions() -> Vec<Value> {
    let id = json!({"type":"string","pattern":"^[1-9][0-9]*$"});
    let entry = json!({"type":"object","properties":{"title":{"type":"string"},"kind":{"type":"string","enum":["income","expense"]},"categoryId":id.clone(),"assetId":id.clone(),"description":{"type":"string"},"amount":{"type":"string","description":"人民币元的十进制文本，例如12.50"},"occurredLocal":{"type":["string","null"],"description":"本地YYYY-MM-DDTHH:mm，不填使用当前时间"}},"required":["title","kind","categoryId","assetId","description","amount"],"additionalProperties":false});
    let rule = json!({"type":["object","null"],"properties":{"unit":{"type":"string","enum":["day","week","month","year"]},"interval":{"type":"integer","minimum":1}},"required":["unit","interval"],"additionalProperties":false});
    [
        ("list_real_money_assets","查询现实资金池及默认资产，只读，不读取银行。",json!({}),vec![]),
        ("list_real_money_categories","查询现实记账分类及修改版本。",json!({"query":{"type":"string"}}),vec![]),
        ("save_real_money_category","新增或修改现实分类；新建时id/revision为null。",json!({"id":{"type":["string","null"]},"revision":{"type":["integer","null"]},"name":{"type":"string"},"icon":{"type":"string"},"direction":{"type":"string","enum":["income","expense","both"]}}),vec!["id","revision","name","icon","direction"]),
        ("delete_real_money_category","删除现实分类，保留历史引用。",json!({"id":id,"revision":{"type":"integer"}}),vec!["id","revision"]),
        ("record_real_money","创建一次性现实账单或周期计划。",json!({"entry":entry,"recurrence":rule,"confirmedRealMoney":{"type":"boolean","description":"用户已明确金额为现实人民币时设true"}}),vec!["entry","confirmedRealMoney"]),
        ("query_real_money_entries","按需查询现实账单及统计，每页12条，金额统计字段为分。",json!({"start":{"type":["string","null"]},"end":{"type":["string","null"]},"direction":{"type":"string","enum":["all","income","expense","adjustment"]},"categoryId":{"type":"string"},"assetId":{"type":"string"},"search":{"type":"string"},"ascending":{"type":"boolean"},"page":{"type":"integer","minimum":1}}),vec![]),
    ].into_iter().map(|(name,description,properties,required)|json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})).collect()
}
pub struct Tools {
    runtime: Runtime,
    lease: Arc<RequestLease>,
    persona: i64,
    session: i64,
    revision: u64,
    prefix: String,
}
impl Tools {
    pub fn new(
        runtime: &Runtime,
        lease: &Arc<RequestLease>,
        persona: i64,
        session: i64,
        revision: u64,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            lease: lease.clone(),
            persona,
            session,
            revision,
            prefix: format!(
                "ai:{}:{}:{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap_or_default(),
                lease.id
            ),
        }
    }
    pub fn execute(&mut self, call: &ToolCall) -> AppResult<Value> {
        let runtime = self.runtime.clone();
        runtime.read(|s, _| {
            self.lease
                .require_conversation_scope(&runtime.requests, self.session, self.persona)?;
            let context = crate::persona::context(&s.connection, s.context_revision)?;
            if context.persona_id != self.persona.to_string()
                || context.session_id != Some(self.session.to_string())
                || context.context_revision != self.revision
            {
                return Err(AppError::new(
                    "CONTEXT_CHANGED",
                    "当前聊天已切换，本次现实记账未执行",
                ));
            }
            runtime.jobs_cancel.check()?;
            let now = runtime.clock.sample()?.utc;
            let name: String = s.connection.query_row(
                "SELECT name FROM personas WHERE id=?1",
                [self.persona],
                |r| r.get(0),
            )?;
            let parse = |v: &Value| {
                serde_json::from_value::<Action>(v.clone())
                    .map_err(|_| invalid("现实记账工具参数无效"))
            };
            let action = match call.name.as_str() {
                "list_real_money_assets" => {
                    let meta = records::overview(&s.connection, now)?;
                    return Ok(meta["assets"].clone());
                }
                "list_real_money_categories" => {
                    let meta = records::overview(&s.connection, now)?;
                    let word = call
                        .arguments
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_lowercase();
                    return Ok(json!(meta["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|v| v["archived"] != true
                            && v["name"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&word))
                        .collect::<Vec<_>>()));
                }
                "query_real_money_entries" => {
                    let q = serde_json::from_value(call.arguments.clone())
                        .map_err(|_| invalid("现实账单查询参数无效"))?;
                    let mut result = records::query(&s.connection, q, now)?;
                    if let Some(object) = result.as_object_mut() {
                        object.remove("balance");
                    }
                    result["currency"] = json!("CNY");
                    result["amountUnit"] = json!("cent");
                    return Ok(result);
                }
                "save_real_money_category" | "delete_real_money_category" => {
                    let mut v = call.arguments.clone();
                    v["type"] = json!(if call.name == "save_real_money_category" {
                        "save_category"
                    } else {
                        "delete_category"
                    });
                    parse(&v)?
                }
                "record_real_money" => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase", deny_unknown_fields)]
                    struct Input {
                        entry: EntryInput,
                        recurrence: Option<Rule>,
                        confirmed_real_money: bool,
                    }
                    let mut input: Input = serde_json::from_value(call.arguments.clone())
                        .map_err(|_| invalid("现实记账参数无效"))?;
                    if !input.confirmed_real_money {
                        return Err(invalid("请先向用户确认这是现实的钱，不是喵币"));
                    }
                    input.entry.recorder = None;
                    Action::SaveEntry {
                        id: None,
                        revision: None,
                        entry: input.entry,
                        recurrence: input.recurrence,
                    }
                }
                _ => return Err(invalid("未开放此现实记账工具")),
            };
            mutate(
                &mut s.connection,
                &format!("{}:{}", self.prefix, call.id),
                action,
                now,
                Actor { source: "ai", name },
            )
        })
    }
}
