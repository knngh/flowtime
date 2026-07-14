use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, FromRow};
use tauri::State;

// ── Structs ──

#[derive(Debug, Serialize, Deserialize)]
pub struct WeeklyReport {
    pub week_start: String,
    pub week_end: String,
    pub total_focus_seconds: i64,
    pub total_tracked_seconds: i64,
    pub tasks_done: i32,
    pub tasks_total: i32,
    pub completion_rate: f64,
    pub avg_interruptions_per_day: f64,
    pub focus_sessions_count: i32,
    pub time_distribution: Vec<TimeDistributionItem>,
    pub high_risk_tasks: Vec<HighRiskTask>,
    pub prev_week_focus_seconds: i64,
    pub prev_week_completion_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeDistributionItem {
    pub category: String,
    pub total_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighRiskTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub deferred_count: i32,
    pub last_deferred_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: String,
    pub total_focus_seconds: i64,
    pub total_tracked_seconds: i64,
    pub tasks_done: i32,
    pub tasks_total: i32,
    pub completion_rate: f64,
    pub focus_sessions_count: i32,
    pub interruptions_blocked: i32,
    pub time_distribution: Vec<TimeDistributionItem>,
}

// ── Helpers ──

/// Parse "YYYY-MM-DD" to NaiveDate. Returns error on invalid format.
fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date format '{}', expected YYYY-MM-DD: {}", s, e))
}

/// Return (week_start, week_end) for the week that contains `date`.
/// Week starts on Monday.
fn week_bounds(mut date: NaiveDate) -> (NaiveDate, NaiveDate) {
    // 0 = Mon ... 6 = Sun  (chrono: 0 = Mon)
    let dow = date.weekday().num_days_from_monday() as i64;
    date = date - chrono::Duration::days(dow);
    let start = date;
    let end = start + chrono::Duration::days(6);
    (start, end)
}

fn iso_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn category_of_app(app: &str) -> &'static str {
    let a = app.to_lowercase();
    if a.contains("code")
        || a.contains("cursor")
        || a.contains("vscode")
        || a.contains("xcode")
        || a.contains("pycharm")
        || a.contains("webstorm")
        || a.contains("rustrover")
        || a.contains("intellij")
        || a.contains("android studio")
        || a.contains("goland")
        || a.contains("clion")
        || a.contains("fleet")
        || a.contains("zed")
        || a.contains("sublime")
        || a.contains("vim")
        || a.contains("neovim")
        || a.contains("emacs")
        || a.contains("notion")
        || a.contains("obsidian")
        || a.contains("terminal")
        || a.contains("iterm")
        || a.contains("kitty")
        || a.contains("warp")
        || a.contains("hyper")
        || a.contains("alacritty")
    {
        "coding"
    } else if a.contains("meet")
        || a.contains("zoom")
        || a.contains("teams")
        || a.contains("腾讯会议")
        || a.contains("飞书")
        || a.contains("钉钉")
        || a.contains("skype")
        || a.contains("webex")
        || a.contains("gotomeeting")
        || a.contains("bluejeans")
        || a.contains("whereby")
    {
        "meeting"
    } else if a.contains("mail")
        || a.contains("outlook")
        || a.contains("thunderbird")
        || a.contains("spark")
        || a.contains("wechat")
        || a.contains("微信")
        || a.contains("slack")
        || a.contains("discord")
        || a.contains("telegram")
        || a.contains("whatsapp")
        || a.contains("signal")
        || a.contains("messenger")
        || a.contains("feishu")
        || a.contains("lark")
        || a.contains("qq")
        || a.contains("line")
    {
        "communication"
    } else if a.contains("figma")
        || a.contains("sketch")
        || a.contains("photoshop")
        || a.contains("illustrator")
        || a.contains("canva")
        || a.contains("blender")
        || a.contains("affinity")
        || a.contains("procreate")
    {
        "design"
    } else if a.contains("spotify")
        || a.contains("music")
        || a.contains("youtube")
        || a.contains("netflix")
        || a.contains("bilibili")
        || a.contains("twitch")
    {
        "entertainment"
    } else if a.contains("chrome")
        || a.contains("firefox")
        || a.contains("safari")
        || a.contains("edge")
        || a.contains("brave")
        || a.contains("arc")
        || a.contains("opera")
    {
        "browsing"
    } else {
        "other"
    }
}

/// Load user-defined app→category overrides from settings (P3-3).
async fn load_category_rules(pool: &SqlitePool) -> std::collections::HashMap<String, String> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'app_category_rules' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    if let Some(json) = raw {
        if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&json) {
            return map;
        }
    }
    std::collections::HashMap::new()
}

/// Resolve a category: user rules first, then the built-in heuristic.
fn categorize(app: &str, rules: &std::collections::HashMap<String, String>) -> String {
    let a = app.to_lowercase();
    if let Some(cat) = rules.get(&a) {
        return cat.clone();
    }
    for (k, v) in rules {
        if !k.is_empty() && a.contains(k) {
            return v.clone();
        }
    }
    category_of_app(app).to_string()
}

// ── Tauri Commands ──

#[tauri::command]
pub async fn get_weekly_report(
    week_start: String,
    state: State<'_, SqlitePool>,
) -> Result<WeeklyReport, String> {
    let pool = state.inner();
    let ws = parse_date(&week_start)?;
    let (_, week_end) = week_bounds(ws);
    let we = week_end;
    let ws_str = iso_date(ws);
    let we_str = iso_date(we);

    // Prev week
    let prev_ws = ws - chrono::Duration::days(7);
    let prev_we = we - chrono::Duration::days(7);
    let prev_ws_str = iso_date(prev_ws);
    let prev_we_str = iso_date(prev_we);

    // ── Focus seconds this week ──
    let focus_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT start_time, end_time FROM focus_sessions \
         WHERE date(start_time) >= ? AND date(start_time) <= ? \
         AND end_time IS NOT NULL",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let focus_sessions_count = focus_rows.len() as i32;
    let mut total_focus_seconds: i64 = 0;
    for (st, et) in &focus_rows {
        if let (Ok(s), Ok(e)) = (
            chrono::DateTime::parse_from_rfc3339(&st),
            chrono::DateTime::parse_from_rfc3339(&et),
        ) {
            total_focus_seconds +=
                (chrono::DateTime::<Utc>::from(e) - chrono::DateTime::<Utc>::from(s))
                    .num_seconds()
                    .max(0);
        }
    }

    // ── Prev week focus ──
    let prev_focus_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT start_time, end_time FROM focus_sessions \
         WHERE date(start_time) >= ? AND date(start_time) <= ? \
         AND end_time IS NOT NULL",
    )
    .bind(&prev_ws_str)
    .bind(&prev_we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut prev_week_focus_seconds: i64 = 0;
    for (st, et) in prev_focus_rows {
        if let (Ok(s), Ok(e)) = (
            chrono::DateTime::parse_from_rfc3339(&st),
            chrono::DateTime::parse_from_rfc3339(&et),
        ) {
            prev_week_focus_seconds +=
                (chrono::DateTime::<Utc>::from(e) - chrono::DateTime::<Utc>::from(s))
                    .num_seconds()
                    .max(0);
        }
    }

    // ── Tasks ──
    let tasks: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM tasks \
         WHERE date(created_at) >= ? AND date(created_at) <= ?",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let tasks_total = tasks.len() as i32;
    let tasks_done = tasks
        .iter()
        .filter(|(_, s)| s.as_str() == "done")
        .count() as i32;
    let completion_rate = if tasks_total > 0 {
        tasks_done as f64 / tasks_total as f64
    } else {
        0.0
    };

    // ── Prev week completion ──
    let prev_tasks: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM tasks \
         WHERE date(created_at) >= ? AND date(created_at) <= ?",
    )
    .bind(&prev_ws_str)
    .bind(&prev_we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let prev_tasks_total = prev_tasks.len() as i32;
    let prev_tasks_done = prev_tasks
        .iter()
        .filter(|(_, s)| s.as_str() == "done")
        .count() as i32;
    let prev_week_completion_rate = if prev_tasks_total > 0 {
        prev_tasks_done as f64 / prev_tasks_total as f64
    } else {
        0.0
    };

    // ── Interruptions (avg per day) ──
    #[derive(FromRow)]
    struct InterruptRow {
        total: i32,
    }

    let interrupt_rows: Vec<InterruptRow> = sqlx::query_as(
        "SELECT COALESCE(SUM(interruption_count), 0) as total \
         FROM focus_sessions \
         WHERE date(start_time) >= ? AND date(start_time) <= ?",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let total_interruptions = interrupt_rows.first().map(|r| r.total).unwrap_or(0);
    // Days with at least one focus session
    let active_days: i32 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT date(start_time)) \
         FROM focus_sessions \
         WHERE date(start_time) >= ? AND date(start_time) <= ?",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    let avg_interruptions_per_day = if active_days > 0 {
        total_interruptions as f64 / active_days as f64
    } else {
        0.0
    };

    // ── Time distribution (window_activity) ──
    let dist_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT app_name, CAST(SUM(duration_seconds) AS INTEGER) as total \
         FROM window_activity \
         WHERE date >= ? AND date <= ? \
         GROUP BY app_name",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    use std::collections::HashMap;
    let rules = load_category_rules(pool).await;
    let mut cat_map: HashMap<String, i64> = HashMap::new();
    for (app, secs) in dist_rows {
        let cat = categorize(&app, &rules);
        *cat_map.entry(cat).or_insert(0) += secs;
    }
    let mut time_distribution: Vec<TimeDistributionItem> = cat_map
        .into_iter()
        .map(|(c, s)| TimeDistributionItem {
            category: c.to_string(),
            total_seconds: s,
        })
        .collect();
    time_distribution.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));

    // ── Total tracked seconds ──
    let total_tracked_seconds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) \
         FROM window_activity \
         WHERE date >= ? AND date <= ?",
    )
    .bind(&ws_str)
    .bind(&we_str)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // ── High risk tasks (deferred 3+ times or status=deferred and created > 3 days ago) ──
    // We detect "high risk" as: status == 'deferred' and (created_at is more than 3 days ago)
    // Also include tasks with status 'deferred' whose title suggests repeated deferral.
    // Simpler heuristic: tasks with status='deferred' and created_at < (today - 3 days)
    let today = Utc::now().date_naive();
    let threshold = iso_date(today - chrono::Duration::days(3));

    // High-risk = deferred tasks older than 3 days, ordered by how many times
    // they've been deferred (real `deferred_count` from migration v6).
    let high_risk: Vec<(String, String, String, String, i32, Option<String>)> = sqlx::query_as(
        "SELECT id, title, status, created_at, deferred_count, last_deferred_at FROM tasks \
         WHERE status = 'deferred' AND date(created_at) < ? \
         ORDER BY deferred_count DESC, created_at ASC \
         LIMIT 20",
    )
    .bind(&threshold)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let high_risk_tasks: Vec<HighRiskTask> = high_risk
        .into_iter()
        .map(|(id, title, status, _created_at, deferred_count, last_deferred_at)| HighRiskTask {
            id,
            title,
            status,
            deferred_count,
            last_deferred_at,
        })
        .collect();

    Ok(WeeklyReport {
        week_start: ws_str,
        week_end: we_str,
        total_focus_seconds,
        total_tracked_seconds,
        tasks_done,
        tasks_total,
        completion_rate,
        avg_interruptions_per_day,
        focus_sessions_count,
        time_distribution,
        high_risk_tasks,
        prev_week_focus_seconds,
        prev_week_completion_rate,
    })
}

#[tauri::command]
pub async fn get_daily_summary(
    date: Option<String>,
    state: State<'_, SqlitePool>,
) -> Result<DailySummary, String> {
    let pool = state.inner();
    let target = date.unwrap_or_else(|| Utc::now().date_naive().to_string());

    // ── Focus seconds ──
    let focus_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT start_time, end_time FROM focus_sessions \
         WHERE date(start_time) = ? AND end_time IS NOT NULL",
    )
    .bind(&target)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut total_focus_seconds: i64 = 0;
    let focus_sessions_count = focus_rows.len() as i32;
    for (st, et) in &focus_rows {
        if let (Ok(s), Ok(e)) = (
            chrono::DateTime::parse_from_rfc3339(st),
            chrono::DateTime::parse_from_rfc3339(et),
        ) {
            total_focus_seconds +=
                (chrono::DateTime::<Utc>::from(e) - chrono::DateTime::<Utc>::from(s))
                    .num_seconds()
                    .max(0);
        }
    }

    // ── Interruptions (real: counts pauses during focus) ──
    let interruptions_blocked: i32 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(interruption_count), 0) \
         FROM focus_sessions \
         WHERE date(start_time) = ?",
    )
    .bind(&target)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // ── Tasks ──
    let tasks: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM tasks WHERE date(created_at) = ?",
    )
    .bind(&target)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let tasks_total = tasks.len() as i32;
    let tasks_done = tasks
        .iter()
        .filter(|(_, s)| s.as_str() == "done")
        .count() as i32;
    let completion_rate = if tasks_total > 0 {
        tasks_done as f64 / tasks_total as f64
    } else {
        0.0
    };

    // ── Time distribution ──
    let dist_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT app_name, CAST(SUM(duration_seconds) AS INTEGER) as total \
         FROM window_activity \
         WHERE date = ? \
         GROUP BY app_name",
    )
    .bind(&target)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    use std::collections::HashMap;
    let rules = load_category_rules(pool).await;
    let mut cat_map: HashMap<String, i64> = HashMap::new();
    for (app, secs) in dist_rows {
        let cat = categorize(&app, &rules);
        *cat_map.entry(cat).or_insert(0) += secs;
    }
    let time_distribution: Vec<TimeDistributionItem> = cat_map
        .into_iter()
        .map(|(c, s)| TimeDistributionItem {
            category: c.to_string(),
            total_seconds: s,
        })
        .collect();

    let total_tracked_seconds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM window_activity WHERE date = ?",
    )
    .bind(&target)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(DailySummary {
        date: target,
        total_focus_seconds,
        total_tracked_seconds,
        tasks_done,
        tasks_total,
        completion_rate,
        focus_sessions_count,
        interruptions_blocked,
        time_distribution,
    })
}

// ── Custom app category rules (P3-3) ──

#[tauri::command]
pub async fn set_app_category(
    app: String,
    category: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    let mut rules = load_category_rules(pool).await;
    rules.insert(app.to_lowercase(), category);
    let json = serde_json::to_string(&rules).map_err(|e| format!("serde error: {}", e))?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('app_category_rules', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_app_categories(
    state: State<'_, SqlitePool>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let pool = state.inner();
    Ok(load_category_rules(pool).await)
}

/// Remove a single app→category rule (P3-3 settings UI).
#[tauri::command]
pub async fn delete_app_category(
    app: String,
    state: State<'_, SqlitePool>,
) -> Result<(), String> {
    let pool = state.inner();
    let mut rules = load_category_rules(pool).await;
    rules.remove(&app.to_lowercase());
    let json = serde_json::to_string(&rules).map_err(|e| format!("serde error: {}", e))?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('app_category_rules', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Defer a task: mark as `deferred`, increment the real `deferred_count`, and
/// record when it was deferred (P1-2).
#[tauri::command]
pub async fn defer_task(task_id: String, state: State<'_, SqlitePool>) -> Result<(), String> {
    let pool = state.inner();
    sqlx::query(
        "UPDATE tasks SET status = 'deferred', deferred_count = deferred_count + 1, \
         last_deferred_at = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&task_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_of_app_coding() {
        assert_eq!(category_of_app("Visual Studio Code"), "coding");
        assert_eq!(category_of_app("Cursor"), "coding");
        assert_eq!(category_of_app("iTerm2"), "coding");
    }

    #[test]
    fn test_category_of_app_meeting() {
        assert_eq!(category_of_app("Zoom"), "meeting");
        assert_eq!(category_of_app("腾讯会议"), "meeting");
    }

    #[test]
    fn test_category_of_app_communication() {
        assert_eq!(category_of_app("WeChat"), "communication");
        assert_eq!(category_of_app("Slack"), "communication");
    }

    #[test]
    fn test_category_of_app_other() {
        assert_eq!(category_of_app("Calculator"), "other");
        assert_eq!(category_of_app("Finder"), "other");
    }

    #[test]
    fn test_week_bounds_monday() {
        // 2026-07-13 is a Monday
        let d = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        let (start, end) = week_bounds(d);
        assert_eq!(start, d);
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 7, 19).unwrap());
    }

    #[test]
    fn test_week_bounds_sunday() {
        // 2026-07-19 is a Sunday → week starts 2026-07-13
        let d = NaiveDate::from_ymd_opt(2026, 7, 19).unwrap();
        let (start, _) = week_bounds(d);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 7, 13).unwrap());
    }

    #[test]
    fn test_categorize_user_rule_wins() {
        use std::collections::HashMap;
        let mut rules = HashMap::new();
        rules.insert("spotify".to_string(), "focus-music".to_string());
        assert_eq!(categorize("Spotify", &rules), "focus-music");
        // Fallback when no rule
        assert_eq!(categorize("Calculator", &rules), "other");
    }
}

