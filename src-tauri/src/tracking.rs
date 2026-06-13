use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WindowActivityRecord {
    pub app_name: String,
    pub window_title: String,
    pub duration_seconds: i32,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AppTimeDistribution {
    pub app_name: String,
    pub total_seconds: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductivityStats {
    pub total_focus_seconds: i64,
    pub total_tracked_seconds: i64,
    pub app_switch_count: i32,
    pub top_apps: Vec<AppTimeDistribution>,
    pub focus_sessions_count: i32,
}

#[tauri::command]
pub async fn track_window_activity(
    app_name: String,
    window_title: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    let today = Utc::now().date_naive().to_string();

    let result = sqlx::query(
        "UPDATE window_activity \
         SET duration_seconds = duration_seconds + 30, recorded_at = ? \
         WHERE date = ? AND app_name = ? AND window_title = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&today)
    .bind(&app_name)
    .bind(&window_title)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO window_activity (date, app_name, window_title, duration_seconds) \
             VALUES (?, ?, ?, 30)",
        )
        .bind(&today)
        .bind(&app_name)
        .bind(&window_title)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to insert window activity: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_daily_time_distribution(
    date: Option<String>,
    state: State<'_, SqlitePool>,
) -> Result<Vec<AppTimeDistribution>, String> {
    let pool = state.inner();
    let target_date = date.unwrap_or_else(|| Utc::now().date_naive().to_string());

    let rows: Vec<AppTimeDistribution> = sqlx::query_as::<_, AppTimeDistribution>(
        "SELECT app_name, CAST(SUM(duration_seconds) AS INTEGER) as total_seconds \
         FROM window_activity \
         WHERE date = ? \
         GROUP BY app_name \
         ORDER BY total_seconds DESC",
    )
    .bind(&target_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    Ok(rows)
}

#[tauri::command]
pub async fn get_productivity_stats(
    date: Option<String>,
    state: State<'_, SqlitePool>,
) -> Result<ProductivityStats, String> {
    let pool = state.inner();
    let target_date = date.unwrap_or_else(|| Utc::now().date_naive().to_string());

    let total_tracked: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM window_activity WHERE date = ?",
    )
    .bind(&target_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    #[derive(FromRow)]
    struct FocusRow {
        start_time: String,
        end_time: String,
    }

    let focus_rows: Vec<FocusRow> = sqlx::query_as::<_, FocusRow>(
        "SELECT start_time, end_time \
         FROM focus_sessions \
         WHERE date(start_time) = ? AND end_time IS NOT NULL",
    )
    .bind(&target_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    let mut total_focus_seconds: i64 = 0;
    for row in &focus_rows {
        if let (Ok(s), Ok(e)) = (
            DateTime::parse_from_rfc3339(&row.start_time),
            DateTime::parse_from_rfc3339(&row.end_time),
        ) {
            total_focus_seconds += (DateTime::<Utc>::from(e) - DateTime::<Utc>::from(s))
                .num_seconds()
                .max(0);
        }
    }

    let switch_count: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT app_name || '|' || window_title) \
         FROM window_activity WHERE date = ?",
    )
    .bind(&target_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    let top_apps: Vec<AppTimeDistribution> = sqlx::query_as::<_, AppTimeDistribution>(
        "SELECT app_name, CAST(SUM(duration_seconds) AS INTEGER) as total_seconds \
         FROM window_activity \
         WHERE date = ? \
         GROUP BY app_name \
         ORDER BY total_seconds DESC \
         LIMIT 10",
    )
    .bind(&target_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    Ok(ProductivityStats {
        total_focus_seconds,
        total_tracked_seconds: total_tracked,
        app_switch_count: switch_count,
        top_apps,
        focus_sessions_count: focus_rows.len() as i32,
    })
}

/// Get the frontmost application name via macOS osascript.
#[tauri::command]
pub fn get_frontmost_app() -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("osascript error: {}", err))
    }
}