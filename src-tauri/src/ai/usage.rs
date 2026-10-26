//! 只记录服务商实际返回的输入/缓存计数；缺失或错误字段不影响回复。
use serde::Serialize;
use serde_json::Value;
#[derive(Clone,Debug,Default,PartialEq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TokenUsage { pub input_tokens:Option<u64>, pub cached_tokens:Option<u64> }
fn count(v:Option<&Value>)->Option<u64>{v?.as_u64().filter(|v|*v<=9_007_199_254_740_991)}
pub fn parse(value:&Value)->Option<TokenUsage>{
 let v=value.get("usage")?;
 let hit=count(v.get("prompt_cache_hit_tokens"));
 let miss=count(v.get("prompt_cache_miss_tokens"));
 let input=count(v.get("prompt_tokens")).or_else(||hit.zip(miss).and_then(|(a,b)|a.checked_add(b)).filter(|v|*v<=9_007_199_254_740_991));
 let cached=count(v.pointer("/prompt_tokens_details/cached_tokens")).or(hit).filter(|n|input.is_none_or(|total|*n<=total));
 if input.is_none()&&cached.is_none(){None}else{Some(TokenUsage{input_tokens:input,cached_tokens:cached})}
}
#[derive(Clone,Debug,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct UsageTotals {pub requests:u32,pub input_requests:u32,pub cached_requests:u32,pub input_tokens:u64,pub cached_tokens:u64}
impl UsageTotals {
 pub fn add(&mut self,usage:Option<&TokenUsage>){
  self.requests+=1;
  if let Some(usage)=usage {
   if let Some(n)=usage.input_tokens{self.input_requests+=1;self.input_tokens=self.input_tokens.saturating_add(n);}
   if let Some(n)=usage.cached_tokens{self.cached_requests+=1;self.cached_tokens=self.cached_tokens.saturating_add(n);}
  }
 }
}
/// 未知兼容接口不添加参数，不为了统计试探、重发或改变缓存策略。
pub fn request(base:&str,body:&mut Value){
 if body["stream"]==true && reqwest::Url::parse(base).ok().is_some_and(|u|matches!(u.host_str(),Some("api.openai.com"|"api.deepseek.com"))){body["stream_options"]=serde_json::json!({"include_usage":true});}
}
#[cfg(test)]
mod tests {
 use super::*;use serde_json::json;
 #[test]fn parses_reported_counts_without_inventing_missing_values(){
  assert_eq!(parse(&json!({"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":0}}})),Some(TokenUsage{input_tokens:Some(100),cached_tokens:Some(0)}));
  assert_eq!(parse(&json!({"usage":{"prompt_cache_hit_tokens":80,"prompt_cache_miss_tokens":20}})),Some(TokenUsage{input_tokens:Some(100),cached_tokens:Some(80)}));
  assert_eq!(parse(&json!({"usage":{"prompt_tokens":100}})).unwrap().cached_tokens,None);
  for v in [json!({}),json!({"usage":null}),json!({"usage":{"prompt_tokens":-1}}),json!({"usage":{"prompt_tokens":"100"}})]{assert!(parse(&v).is_none());}
  assert_eq!(parse(&json!({"usage":{"prompt_tokens":10,"prompt_cache_hit_tokens":20}})).unwrap().cached_tokens,None);
 }
 #[test]fn totals_distinguish_partial_reporting(){let mut t=UsageTotals::default();t.add(parse(&json!({"usage":{"prompt_tokens":100,"prompt_cache_hit_tokens":70}})).as_ref());t.add(None);t.add(parse(&json!({"usage":{"prompt_tokens":200}})).as_ref());assert_eq!((t.requests,t.input_requests,t.cached_requests,t.input_tokens,t.cached_tokens),(3,2,1,300,70));}
 #[test]fn only_known_official_streams_request_usage(){for base in ["https://api.openai.com/v1","https://api.deepseek.com/v1"]{let mut b=json!({"stream":true});request(base,&mut b);assert_eq!(b["stream_options"]["include_usage"],true);}for base in ["https://example.com/v1","https://api.openai.com.example.com"]{let mut b=json!({"stream":true});request(base,&mut b);assert!(b.get("stream_options").is_none());}let mut b=json!({"stream":false});request("https://api.openai.com/v1",&mut b);assert!(b.get("stream_options").is_none());}
}
