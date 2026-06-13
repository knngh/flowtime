use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FocusSessionRow {
    pub id: String,
    pub task_id: Option<String>,
    pub start_time: String,
    pub end_time: Option<String>,
    pub interruptions_blocked: i32,
    pub messages_auto_replied: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveFocusSession {
    pub id: String,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub start_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FocusSessionSummary {
    pub session_id: String,
    pub task_id: Option<String>,
    pub duration_seconds: i64,
    pub interruptions_blocked: i32,
    pub messages_auto_replied: i32,
}

#[derive(FromRow)]
#[allow(dead_code)]
struct IdRow {
    id: String,
}

#[derive(FromRow)]
struct ActiveSessionRow {
    id: String,
    task_id: Option<String>,
    start_time: String,
}

#[derive(FromRow)]
struct TaskTitleRow {
    title: String,
}

#[tauri::command]
pub async fn start_focus_session(
    task_id: Option<String>,
    state: State<'_, SqlitePool>,
) -> Result<String, String> {
    let pool = state.inner();

    let existing: Option<IdRow> =
        sqlx::query_as::<_, IdRow>(
            "SELECT id FROM focus_sessions WHERE end_time IS NULL LIMIT 1"
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    if existing.is_some() {
        return Err("已有活跃的专注会话，请先结束当前专注".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now: DateTime<Utc> = Utc::now();

    sqlx::query(
        "INSERT INTO focus_sessions (id, task_id, start_time) VALUES (?, ?, ?)",
    )
    .bind(&id)
    .bind(&task_id)
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create focus session: {}", e))?;

    Ok(id)
}

#[tauri::command]
pub async fn end_focus_session(
    session_id: String,
    state: State<'_, SqlitePool>,
) -> Result<FocusSessionSummary, String> {
    let pool = state.inner();

    let session: FocusSessionRow = sqlx::query_as::<_, FocusSessionRow>(
        "SELECT id, task_id, start_time, end_time, interruptions_blocked, messages_auto_replied \
         FROM focus_sessions WHERE id = ? AND end_time IS NULL",
    )
    .bind(&session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "专注会话不存在或已结束".to_string())?;

    let end_time: DateTime<Utc> = Utc::now();
    let start_time = DateTime::<Utc>::from(
        DateTime::parse_from_rfc3339(&session.start_time)
            .map_err(|e| format!("Invalid start_time: {}", e))?,
    );
    let duration_seconds = (end_time - start_time).num_seconds().max(0);

    sqlx::query("UPDATE focus_sessions SET end_time = ? WHERE id = ?")
        .bind(end_time.to_rfc3339())
        .bind(&session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to end focus session: {}", e))?;

    Ok(FocusSessionSummary {
        session_id: session.id,
        task_id: session.task_id,
        duration_seconds,
        interruptions_blocked: session.interruptions_blocked,
        messages_auto_replied: session.messages_auto_replied,
    })
}

#[tauri::command]
pub async fn get_active_focus_session(
    state: State<'_, SqlitePool>,
) -> Result<Option<ActiveFocusSession>, String> {
    let pool = state.inner();

    let row: Option<ActiveSessionRow> =
        sqlx::query_as::<_, ActiveSessionRow>(
            "SELECT id, task_id, start_time FROM focus_sessions WHERE end_time IS NULL LIMIT 1"
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    match row {
        Some(r) => {
            let task_title = if let Some(ref tid) = r.task_id {
                let title_row: Option<TaskTitleRow> =
                    sqlx::query_as::<_, TaskTitleRow>(
                        "SELECT title FROM tasks WHERE id = ?"
                    )
                    .bind(tid)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or_else(|e| {
                        log::warn!("[focus] Failed to fetch task title: {}", e);
                        None
                    });
                title_row.map(|t| t.title)
            } else {
                None
            };

            Ok(Some(ActiveFocusSession {
                id: r.id,
                task_id: r.task_id,
                task_title,
                start_time: r.start_time,
            }))
        }
        None => Ok(None),
    }
}
