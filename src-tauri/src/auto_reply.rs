use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use chrono::Utc;
use tauri::State;

// ── LLM helper (shared with llm.rs) ──
use crate::llm_common::chat_completion;

// ── Data structures ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PendingReplyRow {
    pub id: String,
    pub original_message: String,
    pub reply_draft: String,
    pub channel: String,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingReply {
    pub id: String,
    pub original_message: String,
    pub reply_draft: String,
    pub channel: String,
    pub created_at: String,
    pub status: String,
}

// ── Tauri Commands ──

#[tauri::command]
pub async fn generate_auto_reply(
    original_message: String,
    channel: String,
    state: State<'_, SqlitePool>,
) -> Result<PendingReply, String> {
    let pool = state.inner();

    let system_prompt = r#"You are an AI assistant helping the user auto-reply to messages during focus mode.
The user is in deep work and cannot respond immediately.

Generate a polite, concise reply draft in the user's voice. The reply should:
- Acknowledge the message
- Politely explain you're in focus mode / deep work
- Give a rough time when you'll respond (e.g., "later today", "this evening")
- Be warm but brief (1-3 sentences max)
- Match the language of the original message (Chinese ↔ Chinese, English ↔ English)

Output ONLY the reply text, no explanation, no quotes, no markdown."#;

    let user_prompt = format!("Original message:\n{}", original_message);

    let reply_draft = match chat_completion(system_prompt, &user_prompt).await {
        Ok(content) => content.trim().to_string(),
        Err(e) if e == "NO_API_KEY" => {
            if original_message.contains("谢谢") || original_message.contains("感谢") {
                "感谢你的消息！我目前正专注工作中，稍后回复你。".to_string()
            } else if original_message.chars().any(|c| c as u32 > 127) {
                "你好！我目前正专注工作中，稍后回复你。".to_string()
            } else {
                "Thanks for your message! I'm currently in focus mode and will get back to you later today.".to_string()
            }
        }
        Err(e) => {
            log::warn!("Auto-reply LLM error: {}, using fallback", e);
            "I'm currently in focus mode and will reply later. Thanks!".to_string()
        }
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO pending_replies (id, original_message, reply_draft, channel, created_at, status)
         VALUES (?, ?, ?, ?, ?, 'pending')",
    )
    .bind(&id)
    .bind(&original_message)
    .bind(&reply_draft)
    .bind(&channel)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save pending reply: {}", e))?;

    // P1-1: if a focus session is currently active, count this auto-reply as a
    // real "blocked interruption" so the metric is no longer always 0.
    let _ = sqlx::query(
        "UPDATE focus_sessions SET messages_auto_replied = messages_auto_replied + 1 \
         WHERE end_time IS NULL",
    )
    .execute(pool)
    .await;

    Ok(PendingReply {
        id,
        original_message,
        reply_draft,
        channel,
        created_at: now,
        status: "pending".to_string(),
    })
}

#[tauri::command]
pub async fn get_pending_replies(
    state: State<'_, SqlitePool>,
) -> Result<Vec<PendingReply>, String> {
    let pool = state.inner();

    let rows: Vec<PendingReplyRow> = sqlx::query_as::<_, PendingReplyRow>(
        "SELECT id, original_message, reply_draft, channel, created_at, status
         FROM pending_replies
         WHERE status = 'pending'
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|r| PendingReply {
            id: r.id,
            original_message: r.original_message,
            reply_draft: r.reply_draft,
            channel: r.channel,
            created_at: r.created_at,
            status: r.status,
        })
        .collect())
}

#[tauri::command]
pub async fn update_reply_draft(
    reply_id: String,
    new_draft: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    sqlx::query("UPDATE pending_replies SET reply_draft = ? WHERE id = ?")
        .bind(&new_draft)
        .bind(&reply_id)
        .execute(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn mark_reply_sent(
    reply_id: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    sqlx::query("UPDATE pending_replies SET status = 'sent' WHERE id = ?")
        .bind(&reply_id)
        .execute(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn discard_reply(
    reply_id: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    sqlx::query("UPDATE pending_replies SET status = 'discarded' WHERE id = ?")
        .bind(&reply_id)
        .execute(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}