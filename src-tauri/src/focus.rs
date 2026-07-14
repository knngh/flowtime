use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FocusSessionRow {
    pub id: String,
    pub task_id: Option<String>,
    pub start_time: String,
    pub end_time: Option<String>,
    pub interruptions_blocked: i32,
    pub messages_auto_replied: i32,
    pub status: String,
    pub interruption_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveFocusSession {
    pub id: String,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub start_time: String,
    pub status: String,
    pub interruption_count: i32,
    pub elapsed_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FocusSessionSummary {
    pub session_id: String,
    pub task_id: Option<String>,
    pub duration_seconds: i64,
    pub interruptions_blocked: i32,
    pub messages_auto_replied: i32,
    pub status: String,
    pub interruption_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartFocusResult {
    pub session_id: String,
    pub peak_hours_note: Option<String>,
    pub in_peak_hours: bool,
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
    status: String,
    interruption_count: i32,
}

#[derive(FromRow)]
struct TaskTitleRow {
    title: String,
}

#[derive(FromRow)]
struct StartTimeRow {
    start_time: String,
}

// ── Helpers ──

/// Parse hour from a peak range JSON and check membership (pure, testable).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeakRangeData {
    pub start_hour: i32,
    pub end_hour: i32,
}

/// Pure helper: is `hour` inside any of the given peak ranges?
/// Handles ranges that wrap past midnight (start_hour > end_hour), which
/// `find_peak_hours` can produce when the strongest block straddles 00:00.
fn hour_in_peak(hour: i32, ranges: &[PeakRangeData]) -> Option<PeakRangeData> {
    for r in ranges {
        let in_range = if r.start_hour <= r.end_hour {
            hour >= r.start_hour && hour <= r.end_hour
        } else {
            // Wraps midnight, e.g. 23:00–01:00
            hour >= r.start_hour || hour <= r.end_hour
        };
        if in_range {
            return Some(r.clone());
        }
    }
    None
}

/// Check if current time is within user's peak productivity hours.
/// Returns (in_peak, note). Reads `peak_hours_data` from settings (single source
/// of truth, maintained by the learning module via `compute_peak_ranges`).
async fn check_peak_hours(pool: &SqlitePool) -> (bool, Option<String>) {
    let now = Utc::now();
    let current_hour = now.format("%H").to_string().parse::<i32>().unwrap_or(-1);
    if current_hour < 0 {
        return (false, None);
    }

    let peak_data: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'peak_hours_data' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(data) = peak_data {
        if let Ok(ranges) = serde_json::from_str::<Vec<PeakRangeData>>(&data) {
            if let Some(range) = hour_in_peak(current_hour, &ranges) {
                let hour_str = if range.start_hour == range.end_hour {
                    format!("{}:00-{:02}:00", range.start_hour, (range.start_hour + 1) % 24)
                } else {
                    format!("{}:00-{:02}:00", range.start_hour, (range.end_hour + 1) % 24)
                };
                return (true, Some(format!(
                    "💡 现在是你的高效时段（{}），专注效果最佳！",
                    hour_str
                )));
            }
        }
    }

    (false, None)
}

// ── Tauri Commands ──

/// Get the user's peak hours insight (standalone, for frontend to display).
#[tauri::command]
pub async fn get_focus_insight(state: State<'_, SqlitePool>) -> Result<Option<String>, String> {
    let pool = state.inner();
    let (_, note) = check_peak_hours(pool).await;
    Ok(note)
}

#[tauri::command]
pub async fn start_focus_session(
    task_id: Option<String>,
    state: State<'_, SqlitePool>,
) -> Result<StartFocusResult, String> {
    let pool = state.inner();

    // Prevent duplicate: check for active (not completed, not paused) session
    let existing: Option<IdRow> = sqlx::query_as::<_, IdRow>(
        "SELECT id FROM focus_sessions WHERE end_time IS NULL AND status != 'paused' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    if existing.is_some() {
        return Err("已有活跃的专注会话，请先结束或暂停当前专注".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now: DateTime<Utc> = Utc::now();

    sqlx::query(
        "INSERT INTO focus_sessions (id, task_id, start_time, status) VALUES (?, ?, ?, 'active')",
    )
    .bind(&id)
    .bind(&task_id)
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create focus session: {}", e))?;

    // Check peak hours linkage (single algorithm in learning module)
    let (in_peak, peak_note) = check_peak_hours(pool).await;

    Ok(StartFocusResult {
        session_id: id,
        peak_hours_note: peak_note,
        in_peak_hours: in_peak,
    })
}

#[tauri::command]
pub async fn pause_focus_session(
    session_id: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();

    // A pause is a manual interruption — increment interruption_count only.
    let result = sqlx::query(
        "UPDATE focus_sessions SET status = 'paused', interruption_count = interruption_count + 1 \
         WHERE id = ? AND end_time IS NULL AND status = 'active'",
    )
    .bind(&session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("找不到可暂停的专注会话（可能已结束或已暂停）".to_string());
    }

    log::info!("[focus] Session {} paused", session_id);
    Ok(())
}

#[tauri::command]
pub async fn resume_focus_session(
    session_id: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();

    // Check no other active session exists
    let existing: Option<IdRow> = sqlx::query_as::<_, IdRow>(
        "SELECT id FROM focus_sessions WHERE end_time IS NULL AND status = 'active' AND id != ? LIMIT 1",
    )
    .bind(&session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    if existing.is_some() {
        return Err("已有其他活跃的专注会话，请先结束它".to_string());
    }

    let result = sqlx::query(
        "UPDATE focus_sessions SET status = 'active' \
         WHERE id = ? AND end_time IS NULL AND status = 'paused'",
    )
    .bind(&session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("找不到可恢复的暂停会话".to_string());
    }

    log::info!("[focus] Session {} resumed", session_id);
    Ok(())
}

#[tauri::command]
pub async fn end_focus_session(
    app: AppHandle,
    session_id: String,
    state: State<'_, SqlitePool>,
) -> Result<FocusSessionSummary, String> {
    let pool = state.inner();

    let session: FocusSessionRow = sqlx::query_as::<_, FocusSessionRow>(
        "SELECT id, task_id, start_time, end_time, interruptions_blocked, messages_auto_replied, \
                status, interruption_count \
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

    sqlx::query(
        "UPDATE focus_sessions SET end_time = ?, status = 'completed' WHERE id = ?",
    )
    .bind(end_time.to_rfc3339())
    .bind(&session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to end focus session: {}", e))?;

    // Refresh peak-hours cache (single algorithm in learning module).
    crate::learning::cache_peak_hours(pool).await;

    // Desktop notification (P0-4): confirm the completed session.
    let mins = duration_seconds / 60;
    let _ = app
        .notification()
        .builder()
        .title("专注结束")
        .body(format!("本次专注 {} 分钟，继续保持专注节奏！", mins))
        .show();

    Ok(FocusSessionSummary {
        session_id: session.id,
        task_id: session.task_id,
        duration_seconds,
        interruptions_blocked: session.interruptions_blocked,
        messages_auto_replied: session.messages_auto_replied,
        status: "completed".to_string(),
        interruption_count: session.interruption_count,
    })
}

#[tauri::command]
pub async fn get_active_focus_session(
    state: State<'_, SqlitePool>,
) -> Result<Option<ActiveFocusSession>, String> {
    let pool = state.inner();

    let row: Option<ActiveSessionRow> = sqlx::query_as::<_, ActiveSessionRow>(
        "SELECT id, task_id, start_time, status, interruption_count \
         FROM focus_sessions WHERE end_time IS NULL LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    match row {
        Some(r) => {
            let task_title = if let Some(ref tid) = r.task_id {
                let title_row: Option<TaskTitleRow> = sqlx::query_as::<_, TaskTitleRow>(
                    "SELECT title FROM tasks WHERE id = ?",
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

            let start_dt = chrono::DateTime::parse_from_rfc3339(&r.start_time)
                .ok()
                .map(|d| chrono::DateTime::<Utc>::from(d));
            let elapsed = start_dt.map(|s| (Utc::now() - s).num_seconds().max(0));

            Ok(Some(ActiveFocusSession {
                id: r.id,
                task_id: r.task_id,
                task_title,
                start_time: r.start_time,
                status: r.status,
                interruption_count: r.interruption_count,
                elapsed_seconds: elapsed,
            }))
        }
        None => Ok(None),
    }
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hour_in_peak_single() {
        let ranges = vec![PeakRangeData {
            start_hour: 9,
            end_hour: 11,
        }];
        assert!(hour_in_peak(9, &ranges).is_some());
        assert!(hour_in_peak(11, &ranges).is_some());
        assert!(hour_in_peak(12, &ranges).is_none());
    }

    #[test]
    fn test_hour_in_peak_wrap() {
        // {23,1} is inclusive of end_hour (hours 23, 0, 1) — matches
        // find_peak_hours, which returns end_hour as the last covered hour.
        let ranges = vec![PeakRangeData {
            start_hour: 23,
            end_hour: 1,
        }];
        assert!(hour_in_peak(23, &ranges).is_some());
        assert!(hour_in_peak(0, &ranges).is_some());
        assert!(hour_in_peak(1, &ranges).is_some());
        assert!(hour_in_peak(2, &ranges).is_none());
        assert!(hour_in_peak(12, &ranges).is_none());
    }

    #[test]
    fn test_hour_in_peak_multiple() {
        let ranges = vec![
            PeakRangeData {
                start_hour: 9,
                end_hour: 11,
            },
            PeakRangeData {
                start_hour: 14,
                end_hour: 16,
            },
        ];
        assert!(hour_in_peak(15, &ranges).is_some());
        assert!(hour_in_peak(13, &ranges).is_none());
    }
}
