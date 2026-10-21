//! 备份固定字段白名单和单行传输转换；不序列化凭据或派生索引。
use super::*;
use rusqlite::{types::ValueRef, Row};
pub(super) const COLUMNS: &[(&str, &str)] = &[
 ("real_assets","id,name,note,is_default,archived,revision,created_at,updated_at"),
 ("real_categories","id,name,icon,direction,archived,revision,created_at,updated_at"),
 ("real_schedules","id,title,kind,category_id,asset_id,description,amount_cents,anchor_local,unit,interval,generation,next_sequence,status,review_required,recorder,revision,created_at,updated_at"),
 ("real_entries","id,title,kind,category_id,asset_id,description,amount_cents,occurred_local,source,recorder,schedule_id,revision,created_at,updated_at"),
 ("real_occurrences","id,schedule_id,sequence,generation,scheduled_local,snapshot_json,status,entry_id,created_at,resolved_at"),
 ("real_budgets","period,amount_cents,updated_at"),
 ("real_operations","request_id,fingerprint,result_json,created_at"),

    ("food_inventory", "food_id,quantity,updated_at"),
    ("persona_food_settings", "persona_id,default_food_id,favorite_food_id,updated_at"),
    ("food_actions", "action_id,kind,food_id,quantity,persona_id,session_id,food_name,target_name,unit_price,ledger_id,favorite_bonus,intimacy_delta,created_at"),
    ("skills", "id,title,summary,body,revision,created_at,updated_at"),
    ("persona_skills", "persona_id,skill_id,created_at"),

    ("providers", "id,name,base_url,created_at,updated_at"),
    ("provider_roles", "purpose,provider_id,enabled"),
    ("provider_models", "provider_id,purpose,model,max_output_tokens,context_window_override"),
    ("message_images", "id,message_id,filename,image_file,media_type,caption,created_at"),
    ("embedding_providers", "id,name,kind,base_url,model,is_builtin,is_active"),
    ("personas", "id,name,prompt,initial_prompt,self_evolve_enabled,last_evolved_at,color,chat_bg_file,chat_bg_opacity,chat_bg_blur,is_active,is_builtin,expressions,created_at,updated_at,chat_bubble_style,chat_user_text_color,chat_assistant_text_color,pet_image_enabled,pet_image_revision"),
    ("pet_artworks", "id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at"),
    ("user_profiles", "id,name,content,created_at,updated_at"),
    ("sessions", "id,persona_id,user_profile_id,title,created_at,updated_at,memory_revision,craft_tier_1_used,craft_tier_2_used,craft_tier_3_used,intimacy,last_return_at,travel_trigger_remaining_seconds,is_pinned"),
    ("session_pet_states", "session_id,hunger_elapsed_ms,idle_elapsed_ms,idle_branch,sleep_until,last_fed_at,last_interacted_at,state_revision,updated_at,proactive_next_check_idle_ms,last_proactive_sent_at"),
    ("chat_rounds", "id,session_id,status,created_at,completed_at,stop_reason"),
    ("messages", "id,session_id,round_id,role,content,expression,created_at"),
    ("settings", "key,value"),
    ("short_memories", "id,session_id,round_id,content,integration_state,created_at,updated_at"),
    ("long_memories", "id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at"),
    ("long_memory_sources", "long_memory_id,short_memory_id,source_order"),
    ("notebooks", "session_id,content,updated_at"),
    ("reminders", "id,name,kind,schedule,content,status,fire_at,source,created_at"),
    ("reminder_occurrences", "id,reminder_id,scheduled_at,status,delivery_state,completed_at,created_at"),
    ("user_state", "id,coins,updated_at"),
    ("game_plugins", "plugin_id,name,description,author,installed_version,api_version,storage_version,source_type,package_path,package_sha256,repository,homepage,release_page,network_origins_json,is_bundled,package_status,is_enabled,installed_at,updated_at"),
    ("game_permissions", "plugin_id,permission,permission_scope,decision,plugin_version,decided_at"),
    ("game_plugin_settings", "plugin_id,setting_key,value_json,updated_at"),
    ("game_plugin_data", "plugin_id,data_key,value_json,storage_version,updated_at"),
    ("game_runs", "id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,outcome,details_json,coin_delta,started_at,ended_at"),
    ("currency_ledger", "id,delta,reason,ref_id,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at"),
    ("travels", "id,session_id,status,started_at,planned_return_at,actual_return_at,cancelled_at,midway_letter_at,midway_letter_state,return_letter_state,gift_state,journal_state,gift_kind,facts_state,destination,activities_json,facts_version,facts_ready_at,gift_snapshot_json,created_at"),
    ("travel_journals", "id,session_id,travel_id,title,content,created_at,updated_at"),
    ("accessories", "id,session_id,name,display_text,description,cost,source,travel_id,is_worn,created_at,image_file,position_x,position_y,scale,render_layer,layer_order"),
    ("letters", "id,session_id,travel_id,sender,content,reply_to_id,has_been_replied,reply_due_at,reply_state,letter_kind,is_favorite,is_read,created_at"),
    ("souvenirs", "id,session_id,travel_id,content,note,is_favorite,created_at"),
    ("custom_festivals", "id,festival_id,kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,note,source,created_at,updated_at"),
];

pub(super) fn record(row: &Row<'_>, names: &[&str]) -> AppResult<Record> {
    let mut record = Record::new();
    for (index, name) in names.iter().enumerate() {
        let cell = match row.get_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) if *name == "id" || name.ends_with("_id") => {
                if n <= 0 {
                    return Err(invalid());
                }
                Value::String(n.to_string())
            }
            ValueRef::Integer(n) => Value::from(n),
            ValueRef::Real(n) => {
                Value::Number(serde_json::Number::from_f64(n).ok_or_else(invalid)?)
            }
            ValueRef::Text(raw) => {
                Value::String(std::str::from_utf8(raw).map_err(|_| invalid())?.into())
            }
            ValueRef::Blob(_) => return Err(invalid()),
        };
        record.insert((*name).into(), cell);
    }
    Ok(record)
}
pub(super) fn setting_allowed(key: &str) -> bool {
    crate::config::DEFAULTS
        .iter()
        .any(|(known, _)| *known == key)
        || [
            "bored_minutes", // 仅兼容旧备份，暂存后转换为共同阈值
            "sleep_idle_minutes",
            "provider_roles_revision",
            "theme_mode",
            "current_session_id",
            "last_sign_in_date",
            "window_pos_x",
            "window_pos_y",
            "image_generation_config",
            "web_search_config",
            "reminder_email_config",
        ]
        .contains(&key)
}
pub(super) fn public_config(raw: &str, image: bool) -> AppResult<String> {
    let mut config: Value = serde_json::from_str(raw).map_err(|_| invalid())?;
    let object = config.as_object_mut().ok_or_else(invalid)?;
    object.retain(|key, _| {
        [
            "version",
            "revision",
            "nextId",
            "enabled",
            "activeId",
            "providers",
        ]
        .contains(&key.as_str())
    });
    for provider in object
        .get_mut("providers")
        .and_then(Value::as_array_mut)
        .ok_or_else(invalid)?
    {
        let entry = provider.as_object_mut().ok_or_else(invalid)?;
        entry.retain(|key, _| {
            if image {
                ["id", "name", "protocol", "baseUrl", "model"].contains(&key.as_str())
            } else {
                ["id", "name", "kind", "baseUrl"].contains(&key.as_str())
            }
        });
    }
    serde_json::to_string(&config).map_err(|_| invalid())
}
