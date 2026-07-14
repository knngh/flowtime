use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

// ── Export/Import structures ──

#[derive(Debug, Serialize, Deserialize)]
pub struct DataExport {
    pub version: String,
    pub exported_at: String,
    pub projects: Vec<ProjectExport>,
    pub tasks: Vec<TaskExport>,
    pub focus_sessions: Vec<FocusSessionExport>,
    pub settings: Vec<SettingExport>,
    pub window_activity: Vec<WindowActivityExport>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectExport {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskExport {
    pub id: String,
    pub title: String,
    pub priority: String,
    pub estimated_duration_min: i32,
    pub source: String,
    pub source_url: Option<String>,
    pub project_id: Option<String>,
    pub tags: String,
    pub status: String,
    pub scheduled_start: Option<String>,
    pub scheduled_end: Option<String>,
    pub actual_start: Option<String>,
    pub actual_end: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FocusSessionExport {
    pub id: String,
    pub task_id: Option<String>,
    pub start_time: String,
    pub end_time: Option<String>,
    pub interruptions_blocked: i32,
    pub messages_auto_replied: i32,
    pub status: String,
    pub interruption_count: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SettingExport {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WindowActivityExport {
    pub id: i64,
    pub date: String,
    pub app_name: String,
    pub window_title: String,
    pub duration_seconds: i32,
    pub recorded_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub projects_imported: i32,
    pub tasks_imported: i32,
    pub sessions_imported: i32,
    pub settings_imported: i32,
}

// ── Export ──

#[tauri::command]
pub async fn export_all_data(
    state: State<'_, SqlitePool>,
) -> Result<String, String> {
    let pool = state.inner();

    let projects: Vec<ProjectExport> = sqlx::query_as(
        "SELECT id, name, color, created_at, updated_at FROM projects ORDER BY created_at"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to export projects: {}", e))?;

    let tasks: Vec<TaskExport> = sqlx::query_as(
        "SELECT id, title, priority, estimated_duration_min, source, source_url, \
                project_id, tags, status, scheduled_start, scheduled_end, \
                actual_start, actual_end, created_at, updated_at \
         FROM tasks ORDER BY created_at"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to export tasks: {}", e))?;

    let focus_sessions: Vec<FocusSessionExport> = sqlx::query_as(
        "SELECT id, task_id, start_time, end_time, interruptions_blocked, \
                messages_auto_replied, \
                COALESCE(status, 'active') as status, \
                COALESCE(interruption_count, 0) as interruption_count \
         FROM focus_sessions ORDER BY start_time"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to export focus sessions: {}", e))?;

    let settings: Vec<SettingExport> = sqlx::query_as(
        "SELECT key, value FROM settings ORDER BY key"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to export settings: {}", e))?;

    let window_activity: Vec<WindowActivityExport> = sqlx::query_as(
        "SELECT id, date, app_name, window_title, duration_seconds, recorded_at \
         FROM window_activity ORDER BY date, recorded_at"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to export window activity: {}", e))?;

    let export = DataExport {
        version: "1.0.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        projects,
        tasks,
        focus_sessions,
        settings,
        window_activity,
    };

    serde_json::to_string_pretty(&export)
        .map_err(|e| format!("Failed to serialize: {}", e))
}

// ── Import ──

#[tauri::command]
pub async fn import_all_data(
    json_data: String,
    state: State<'_, SqlitePool>,
) -> Result<ImportResult, String> {
    let pool = state.inner();
    let export: DataExport = serde_json::from_str(&json_data)
        .map_err(|e| format!("Invalid JSON data: {}", e))?;

    let mut result = ImportResult {
        projects_imported: 0,
        tasks_imported: 0,
        sessions_imported: 0,
        settings_imported: 0,
    };

    // Import projects (skip duplicates)
    for p in &export.projects {
        let existing_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projects WHERE id = ?"
        )
        .bind(&p.id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if existing_count == 0 {
            sqlx::query(
                "INSERT INTO projects (id, name, color, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&p.id)
            .bind(&p.name)
            .bind(&p.color)
            .bind(&p.created_at)
            .bind(&p.updated_at)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to import project {}: {}", p.id, e))?;
            result.projects_imported += 1;
        }
    }

    // Import tasks (skip duplicates)
    for t in &export.tasks {
        let existing_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE id = ?"
        )
        .bind(&t.id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if existing_count == 0 {
            sqlx::query(
                "INSERT INTO tasks (id, title, priority, estimated_duration_min, source, \
                 source_url, project_id, tags, status, scheduled_start, scheduled_end, \
                 actual_start, actual_end, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&t.id).bind(&t.title).bind(&t.priority).bind(t.estimated_duration_min)
            .bind(&t.source).bind(&t.source_url).bind(&t.project_id).bind(&t.tags)
            .bind(&t.status).bind(&t.scheduled_start).bind(&t.scheduled_end)
            .bind(&t.actual_start).bind(&t.actual_end).bind(&t.created_at).bind(&t.updated_at)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to import task {}: {}", t.id, e))?;
            result.tasks_imported += 1;
        }
    }

    // Import focus sessions
    for s in &export.focus_sessions {
        let existing_count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM focus_sessions WHERE id = ?"
        )
        .bind(&s.id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if existing_count == 0 {
            sqlx::query(
                "INSERT INTO focus_sessions (id, task_id, start_time, end_time, \
                 interruptions_blocked, messages_auto_replied, status, interruption_count) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&s.id).bind(&s.task_id).bind(&s.start_time).bind(&s.end_time)
            .bind(s.interruptions_blocked).bind(s.messages_auto_replied)
            .bind(&s.status).bind(s.interruption_count)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to import session {}: {}", s.id, e))?;
            result.sessions_imported += 1;
        }
    }

    // Import settings (upsert)
    for setting in &export.settings {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value"
        )
        .bind(&setting.key)
        .bind(&setting.value)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to import setting {}: {}", setting.key, e))?;
        result.settings_imported += 1;
    }

    log::info!(
        "[import] Imported {} projects, {} tasks, {} sessions, {} settings",
        result.projects_imported,
        result.tasks_imported,
        result.sessions_imported,
        result.settings_imported,
    );

    Ok(result)
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_import_roundtrip() {
        let export = DataExport {
            version: "1.0.0".to_string(),
            exported_at: "2026-06-13T08:00:00Z".to_string(),
            projects: vec![ProjectExport {
                id: "p1".to_string(),
                name: "Work".to_string(),
                color: "#3B82F6".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            tasks: vec![TaskExport {
                id: "t1".to_string(),
                title: "Task".to_string(),
                priority: "A".to_string(),
                estimated_duration_min: 60,
                source: "manual".to_string(),
                source_url: None,
                project_id: Some("p1".to_string()),
                tags: "[]".to_string(),
                status: "todo".to_string(),
                scheduled_start: None,
                scheduled_end: None,
                actual_start: None,
                actual_end: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            focus_sessions: vec![FocusSessionExport {
                id: "f1".to_string(),
                task_id: None,
                start_time: "2026-01-01T09:00:00Z".to_string(),
                end_time: None,
                interruptions_blocked: 0,
                messages_auto_replied: 0,
                status: "active".to_string(),
                interruption_count: 0,
            }],
            settings: vec![SettingExport {
                key: "k".to_string(),
                value: "v".to_string(),
            }],
            window_activity: vec![WindowActivityExport {
                id: 1,
                date: "2026-01-01".to_string(),
                app_name: "Code".to_string(),
                window_title: "".to_string(),
                duration_seconds: 30,
                recorded_at: "2026-01-01T09:00:30Z".to_string(),
            }],
        };

        let json = serde_json::to_string_pretty(&export).unwrap();
        let back: DataExport = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string_pretty(&back).unwrap();
        assert_eq!(json, json2);
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.tasks.len(), 1);
        assert_eq!(back.tasks[0].project_id, Some("p1".to_string()));
        assert_eq!(back.focus_sessions[0].status, "active");
    }
}
