use chrono::Utc;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_sql::{Builder, Migration, MigrationKind};

mod api;
mod auto_reply;
mod data_io;
mod focus;
mod integrations;
mod learning;
mod llm;
mod llm_common;
mod review;
mod tracking;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Validate critical environment variables at startup and log warnings.
fn validate_environment() {
    let ollama_base = std::env::var("OLLAMA_API_BASE")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_default();

    if !ollama_base.is_empty() {
        log::info!("✅ 本地 LLM (Ollama) 已配置: {}", ollama_base);
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());
        log::info!("   Ollama 模型: {}", model);
    }

    let checks = [
        ("OPENAI_API_KEY", "云端 AI 任务解析和排序功能需要"),
        ("OLLAMA_API_BASE", "本地 LLM 端点（替代 OpenAI）"),
        ("OPENAI_API_BASE", "自定义 API 端点（可选，默认 OpenAI）"),
        ("OPENAI_MODEL", "AI 模型选择（可选，默认 gpt-4o-mini）"),
        ("GITHUB_TOKEN", "GitHub Issues 集成需要"),
        ("LINEAR_API_KEY", "Linear Issues 集成需要"),
        ("FEISHU_APP_ID", "飞书日历集成需要"),
        ("FEISHU_APP_SECRET", "飞书日历集成需要"),
    ];

    let mut missing_critical = false;
    let mut optional_missing = Vec::new();

    for (var, desc) in &checks {
        match std::env::var(var) {
            Ok(val) if !val.is_empty() => {
                if *var != "OLLAMA_API_BASE" || ollama_base.is_empty() {
                    log::info!("✅ 环境变量 {} 已设置（{}）", var, desc);
                }
            }
            _ => {
                if *var == "OPENAI_API_KEY" && ollama_base.is_empty() {
                    log::warn!("⚠️  关键环境变量 OPENAI_API_KEY 未设置 —— {}", desc);
                    missing_critical = true;
                } else if *var != "OLLAMA_API_BASE" {
                    optional_missing.push((var, desc));
                }
            }
        }
    }

    if missing_critical {
        log::warn!(
            "⚠️  缺少 OPENAI_API_KEY 且未配置 Ollama，AI 功能将使用启发式回退算法"
        );
    } else if !ollama_base.is_empty() {
        log::info!("✅ 使用本地 Ollama 作为 AI 后端，无需 OpenAI API Key");
    }

    if !optional_missing.is_empty() {
        log::info!("ℹ️  以下可选集成未配置（不影响核心功能）：");
        for (var, desc) in optional_missing {
            log::info!("   • {} —— {}", var, desc);
        }
    }
}

/// Background loop: remind the user about tasks due within the next 2 hours
/// (P0-4). `scheduled_end` is populated by the AI scheduler (P3-1).
async fn run_deadline_checker(pool: sqlx::SqlitePool, app: tauri::AppHandle) {
    use std::collections::HashSet;
    let mut notified: HashSet<String> = HashSet::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(900));
    loop {
        interval.tick().await;
        let now = Utc::now();
        let now_iso = now.to_rfc3339();
        let soon = (now + chrono::Duration::hours(2)).to_rfc3339();

        #[derive(sqlx::FromRow)]
        struct DueRow {
            id: String,
            title: String,
        }

        let rows: Vec<DueRow> = sqlx::query_as(
            "SELECT id, title FROM tasks \
             WHERE scheduled_end IS NOT NULL AND status != 'done' \
               AND scheduled_end >= ? AND scheduled_end <= ?",
        )
        .bind(&now_iso)
        .bind(&soon)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for r in rows {
            if notified.contains(&r.id) {
                continue;
            }
            notified.insert(r.id.clone());
            let _ = app
                .notification()
                .builder()
                .title("任务即将截止")
                .body(format!("「{}」将在 2 小时内到截止时间", r.title))
                .show();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = vec![
        Migration {
            version: 1,
            description: "create projects and tasks tables",
            kind: MigrationKind::Up,
            sql: "
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL DEFAULT '#3B82F6',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                priority TEXT NOT NULL DEFAULT 'B',
                estimated_duration_min INTEGER NOT NULL DEFAULT 30,
                source TEXT NOT NULL DEFAULT 'manual',
                source_url TEXT,
                project_id TEXT,
                tags TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'todo',
                scheduled_start TEXT,
                scheduled_end TEXT,
                actual_start TEXT,
                actual_end TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_scheduled_start ON tasks(scheduled_start);
        ",
        },
        Migration {
            version: 2,
            description: "create focus_sessions and window_activity tables",
            kind: MigrationKind::Up,
            sql: "
            CREATE TABLE IF NOT EXISTS focus_sessions (
                id TEXT PRIMARY KEY,
                task_id TEXT,
                start_time TEXT NOT NULL DEFAULT (datetime('now')),
                end_time TEXT,
                interruptions_blocked INTEGER NOT NULL DEFAULT 0,
                messages_auto_replied INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_focus_sessions_task_id ON focus_sessions(task_id);
            CREATE INDEX IF NOT EXISTS idx_focus_sessions_start_time ON focus_sessions(start_time);

            CREATE TABLE IF NOT EXISTS window_activity (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL DEFAULT (date('now')),
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL DEFAULT '',
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_window_activity_date ON window_activity(date);
            CREATE INDEX IF NOT EXISTS idx_window_activity_app_name ON window_activity(app_name);
        ",
        },
        Migration {
            version: 3,
            description: "create pending_replies table for M6 auto-reply",
            kind: MigrationKind::Up,
            sql: "
            CREATE TABLE IF NOT EXISTS pending_replies (
                id TEXT PRIMARY KEY,
                original_message TEXT NOT NULL,
                reply_draft TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT 'unknown',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                status TEXT NOT NULL DEFAULT 'pending'
            );
            CREATE INDEX IF NOT EXISTS idx_pending_replies_status ON pending_replies(status);
            CREATE INDEX IF NOT EXISTS idx_pending_replies_created_at ON pending_replies(created_at);
        ",
        },
        Migration {
            version: 4,
            description: "create settings table for M8 behavior learning",
            kind: MigrationKind::Up,
            sql: "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ",
        },
        Migration {
            version: 5,
            description: "add status and interruption count to focus_sessions",
            kind: MigrationKind::Up,
            sql: "
            ALTER TABLE focus_sessions ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
            ALTER TABLE focus_sessions ADD COLUMN interruption_count INTEGER NOT NULL DEFAULT 0;
            UPDATE focus_sessions SET status = 'completed' WHERE end_time IS NOT NULL;
        ",
        },
        Migration {
            version: 6,
            description: "add deferred_count and last_deferred_at to tasks",
            kind: MigrationKind::Up,
            sql: "
            ALTER TABLE tasks ADD COLUMN deferred_count INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE tasks ADD COLUMN last_deferred_at TEXT;
        ",
        },
    ];

    let sql_plugin = Builder::default()
        .add_migrations("sqlite:flowtime.db", migrations)
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        // Shortcuts are registered from the frontend (plugin-global-shortcut JS API)
        // so the handler logic lives with the UI and is version-stable.
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(sql_plugin)
        .setup(|app| {
            // Initialize logger
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();

            log::info!("🚀 Flowtime starting up...");
            validate_environment();

            let pool = app.state::<sqlx::SqlitePool>().inner().clone();
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn({
                let pool = pool.clone();
                async move {
                    api::start_api_server(pool).await;
                }
            });

            tauri::async_runtime::spawn({
                let pool = pool.clone();
                let app_handle = app_handle.clone();
                async move {
                    run_deadline_checker(pool, app_handle).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // M3: Focus mode
            focus::start_focus_session,
            focus::pause_focus_session,
            focus::resume_focus_session,
            focus::end_focus_session,
            focus::get_active_focus_session,
            focus::get_focus_insight,
            // M2: LLM scheduling
            llm::parse_natural_language,
            llm::suggest_schedule,
            // M4: Behavior tracking
            tracking::track_window_activity,
            tracking::get_daily_time_distribution,
            tracking::get_productivity_stats,
            tracking::get_frontmost_app,
            // M5: External integrations
            integrations::fetch_github_issues,
            integrations::fetch_linear_issues,
            integrations::fetch_feishu_events,
            integrations::import_external_tasks,
            // M6: AI auto-reply
            auto_reply::generate_auto_reply,
            auto_reply::get_pending_replies,
            auto_reply::update_reply_draft,
            auto_reply::mark_reply_sent,
            auto_reply::discard_reply,
            // M7: Review dashboard
            review::get_weekly_report,
            review::get_daily_summary,
            review::set_app_category,
            review::get_app_categories,
            review::delete_app_category,
            review::defer_task,
            // M8: Behavior learning
            learning::get_efficiency_pattern,
            learning::calibrate_estimate,
            learning::get_peak_hours,
            learning::get_calibration_ratio,
            // Data import/export
            data_io::export_all_data,
            data_io::import_all_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
