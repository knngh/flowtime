use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::net::TcpListener;

// ── Response types ──

#[derive(Debug, Serialize)]
pub struct TodayTask {
    pub id: String,
    pub title: String,
    pub priority: String,
    pub estimated_duration_min: i32,
    pub status: String,
    pub scheduled_start: Option<String>,
    pub scheduled_end: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub project_color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TodaySummary {
    pub date: String,
    pub tasks_done: i32,
    pub tasks_total: i32,
    pub tasks_remaining: i32,
    pub total_focus_seconds: i64,
    pub focus_sessions_count: i32,
}

#[derive(Debug, Serialize)]
pub struct FocusStatus {
    pub in_focus: bool,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub start_time: Option<String>,
    pub elapsed_seconds: Option<i64>,
}

// ── Handlers ──

async fn today_tasks(State(pool): State<SqlitePool>) -> Result<Json<Vec<TodayTask>>, StatusCode> {
    let today = Utc::now().date_naive().to_string();

    #[derive(sqlx::FromRow)]
    struct TaskRow {
        id: String,
        title: String,
        priority: String,
        estimated_duration_min: i32,
        status: String,
        scheduled_start: Option<String>,
        scheduled_end: Option<String>,
        project_id: Option<String>,
        project_name: Option<String>,
        project_color: Option<String>,
    }

    let rows: Vec<TaskRow> = sqlx::query_as(
        "SELECT t.id, t.title, t.priority, t.estimated_duration_min, t.status,
                t.scheduled_start, t.scheduled_end, t.project_id,
                p.name AS project_name, p.color AS project_color
         FROM tasks t
         LEFT JOIN projects p ON t.project_id = p.id
         WHERE (t.scheduled_start IS NOT NULL AND date(t.scheduled_start) = ?)
            OR (t.scheduled_start IS NULL AND date(t.created_at) = ?)
         ORDER BY
            CASE t.priority WHEN 'A' THEN 0 WHEN 'B' THEN 1 WHEN 'C' THEN 2 END,
            t.scheduled_start ASC",
    )
    .bind(&today)
    .bind(&today)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tasks: Vec<TodayTask> = rows
        .into_iter()
        .map(|r| TodayTask {
            id: r.id,
            title: r.title,
            priority: r.priority,
            estimated_duration_min: r.estimated_duration_min,
            status: r.status,
            scheduled_start: r.scheduled_start,
            scheduled_end: r.scheduled_end,
            project_id: r.project_id,
            project_name: r.project_name,
            project_color: r.project_color,
        })
        .collect();

    Ok(Json(tasks))
}

async fn today_summary(State(pool): State<SqlitePool>) -> Result<Json<TodaySummary>, StatusCode> {
    let today = Utc::now().date_naive().to_string();

    // Tasks done today
    let tasks_done: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE status = 'done' AND date(created_at) = ?",
    )
    .bind(&today)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tasks_total: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE date(created_at) = ?",
    )
    .bind(&today)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tasks_remaining: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE status != 'done'
         AND ( (scheduled_start IS NOT NULL AND date(scheduled_start) = ?)
            OR (scheduled_start IS NULL AND date(created_at) = ?) )",
    )
    .bind(&today)
    .bind(&today)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Focus seconds today
    let focus_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT start_time, end_time FROM focus_sessions
         WHERE date(start_time) = ? AND end_time IS NOT NULL",
    )
    .bind(&today)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let focus_sessions_count = focus_rows.len() as i32;
    let mut total_focus_seconds: i64 = 0;
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

    Ok(Json(TodaySummary {
        date: today,
        tasks_done,
        tasks_total,
        tasks_remaining,
        total_focus_seconds,
        focus_sessions_count,
    }))
}

async fn focus_status(State(pool): State<SqlitePool>) -> Result<Json<FocusStatus>, StatusCode> {
    #[derive(sqlx::FromRow)]
    #[allow(dead_code)]
    struct ActiveRow {
        id: String,
        task_id: Option<String>,
        start_time: String,
    }

    let active: Option<ActiveRow> = sqlx::query_as(
        "SELECT id, task_id, start_time FROM focus_sessions WHERE end_time IS NULL LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match active {
        Some(session) => {
            let task_title: Option<String> = if let Some(ref tid) = session.task_id {
                    sqlx::query_scalar("SELECT title FROM tasks WHERE id = ?")
                    .bind(tid)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or_else(|e| {
                        log::warn!("[api] Failed to fetch task title: {}", e);
                        None
                    })
            } else {
                None
            };

            let start_dt = chrono::DateTime::parse_from_rfc3339(&session.start_time)
                .ok()
                .map(|d| chrono::DateTime::<Utc>::from(d));
            let elapsed = start_dt
                .map(|s| (Utc::now() - s).num_seconds().max(0));

            Ok(Json(FocusStatus {
                in_focus: true,
                task_id: session.task_id,
                task_title,
                start_time: Some(session.start_time),
                elapsed_seconds: elapsed,
            }))
        }
        None => Ok(Json(FocusStatus {
            in_focus: false,
            task_id: None,
            task_title: None,
            start_time: None,
            elapsed_seconds: None,
        })),
    }
}

// ── Server startup ──

async fn health() -> &'static str {
    "ok"
}

pub async fn start_api_server(pool: SqlitePool) {
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/today/tasks", get(today_tasks))
        .route("/api/today/summary", get(today_summary))
        .route("/api/focus/status", get(focus_status))
        .with_state(pool);

    // Bind to random port
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            log::error!("[api] Failed to bind to port: {}", e);
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            log::error!("[api] Failed to get local address: {}", e);
            return;
        }
    };

    // Write port to file for mobile to read
    if let Some(home) = dirs_next::home_dir() {
        let port_file_path = home.join(".flowtime-api-port");
        if let Err(e) = std::fs::write(&port_file_path, port.to_string()) {
            log::warn!("[api] Failed to write port file: {}", e);
        }
    }

    log::info!("📱 Flowtime API server listening on http://127.0.0.1:{}", port);

    if let Err(e) = axum::serve(listener, app).await {
        log::error!("[api] Server error: {}", e);
    }
}
