use super::*;
use crate::{ai::provider::ToolCall, runtime::Runtime, state::RequestLease};
use std::sync::Arc;
pub const RULES:&str="现实记账是独立的人民币账本，与喵币、游戏钱包完全无关。只有用户明确要求记账时才写入；提到价格、咨询、举例、假设不等于记账。用户明确说现实的钱、人民币、RMB/CNY等可直接记；如果只说花了多少、记一笔而金额性质不清，必须先明确询问是不是现实的钱，未得到确认不得调用record_real_money。确认现实记账后confirmedRealMoney才可为true。金额使用元的十进制字符串，不能输入分或喵币，不能捏造金额、资产或周期。先按需查分类和资产；未指定资产可用唯一默认资产，完成时说明。分类不合适时可按用户意图新增；修改/删除分类先读取id与revision。记账时间不明确时默认当前本地时间，用户提及昨天等应使用真实日期；含糊的日期或周期要询问。周期用每N天/周/月/年，首次日期是anchor，短月取月末；关闭期间漏期必须由用户在现实记账界面确认，不能替用户批量补记。工具成功后才说记好，报告标题、金额、收入/支出、分类、资产、日期及是否创建周期。查询只读取解决当前问题所需的范围，不把全量财务资料当作默认聊天上下文。";
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
        ("save_real_money_category","新增或修改现实分类。新建id/revision为null，修改前先读。",json!({"id":{"type":["string","null"]},"revision":{"type":["integer","null"]},"name":{"type":"string"},"icon":{"type":"string"},"direction":{"type":"string","enum":["income","expense","both"]}}),vec!["id","revision","name","icon","direction"]),
        ("delete_real_money_category","删除现实分类，历史引用保留；先读取版本。",json!({"id":id,"revision":{"type":"integer"}}),vec!["id","revision"]),
        ("record_real_money","仅明确现实金额且用户要求记账后，创建一次性账单或周期计划。",json!({"entry":entry,"recurrence":rule,"confirmedRealMoney":{"type":"boolean","description":"只在用户明确现实金额或询问得到确认后为true"}}),vec!["entry","confirmedRealMoney"]),
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
