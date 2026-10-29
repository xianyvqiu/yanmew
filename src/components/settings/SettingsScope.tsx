const scopes:Record<string,{label:string;description:string}>={
  "聊天表情包库":{label:"全局表情包库",description:"库、表情包和标签统一管理，可绑定给多个人设。伙伴按需在聊天消息中发送，和桌宠外观表情分别管理。"},
  "通用":{label:"全局共享",description:"主题、聊天工具、邮件、自启与备份设置，对所有人设和会话生效。"},
  "提供商":{label:"全局共享",description:"连接、Key和模型用途由所有人设共用，切换人设不会切换这组服务设置。"},
  "人设":{label:"全局人设库",description:"这里管理整个颜喵可用的人设。每个人设分别保存自己的设定、外观和表情库，修改其中一位不会改动其他人设。"},
  "用户设定":{label:"全局用户设定库",description:"用户设定统一保存在这里，各会话可以选择绑定不同条目；绑定只决定使用哪一份，不会搬动资料。默认绑定仅用于新会话。"},
  "提醒":{label:"全局共享",description:"提醒规则和记录由所有人设共用；到点时，提醒话语记入触发时的当前会话。"},
  "时间与天气":{label:"全局天气服务",description:"地点、天气与气象资讯由所有人设共用。角色关怀的口吻随当前人设变化，不改变天气资料的保存位置。"},
  "记忆":{label:"全局记忆参数",description:"本页只设置所有会话共用的生成与上下文参数。具体聊天、记忆、记事本和游记在各自会话中查看和保存。"},
  "Skill 仓库":{label:"全局 Skill 仓库",description:"Skill统一保存在这里，各人设可绑定需要的条目。绑定是使用关系，不会把原文搬到人设或会话里。"},
  "喵币与签到":{label:"全局共享",description:"所有人设共用同一个钱包和每日签到记录。"},
  "现实记账":{label:"全局共享",description:"现实账本、资产、分类和预算由所有人设共用，与喵币钱包分别记账。"},
  "小游戏插件":{label:"全局共享",description:"插件、授权、设置与存档按插件保存；换人设或会话不会重开进度。"},
  "食物":{label:"全局食物库存",description:"所有人设共用这里的食物库存和购买记录。选择某个人设喜欢什么、默认吃什么，只调整该人设的偏好。"},
  "喵币账单":{label:"全局共享",description:"这里汇总所有人设的喵币收入与支出，包括食物、饰品和小游戏。"},
  "日历":{label:"全局共享",description:"自定义日期、节日与周期计划在所有人设之间共享。"},
  "桌宠状态":{label:"当前会话",description:"亲密度、饥饿和日常生活状态保存在当前会话中。"},
  "桌宠自定义创作":{label:"当前人设",description:"默认形象与表情库属于当前人设，同一人设的不同会话共用。"},
  "时间与桌宠":{label:"全局共享",description:"生活节奏、出行等待和聊天自动收起等参数对所有人设生效，已抽取的当前倒计时按原规则保留。"},
  "出行":{label:"当前会话",description:"出行、信件、伴手礼和旅行游记属于当前会话，不与其他会话混合。"},
  "饰品":{label:"当前会话",description:"饰品和佩戴方案属于当前会话；制作、出售使用全局喵币钱包。"},
};
export default function SettingsScope({page}:{page:string}){
  const scope=scopes[page];if(!scope)return null;
  return <aside className="ym-settings-scope" aria-label="数据归属"><strong>{scope.label}</strong><p>{scope.description}</p></aside>;
}
