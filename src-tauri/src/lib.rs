use tauri::Manager;
use tauri_plugin_sql::{Builder, Migration, MigrationKind};

mod api;
mod auto_reply;
mod focus;
mod integrations;
mod learning;
mod llm;
mod review;
mod tracking;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Validate critical environment variables at startup and log warnings.
fn validate_environment() {
    let checks = [
        ("OPENAI_API_KEY", "AI 任务解析和排序功能需要"),
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
                log::info!("✅ 环境变量 {} 已设置（{}）", var, desc);
            }
            _ => {
                if *var == "OPENAI_API_KEY" {
                    log::warn!("⚠️  关键环境变量 {} 未设置 —— {}", var, desc);
                    missing_critical = true;
                } else {
                    optional_missing.push((var, desc));
                }
            }
        }
    }

    if missing_critical {
        log::warn!(
            "⚠️  缺少 OPENAI_API_KEY，AI 功能将使用启发式回退算法"
        );
    }

    if !optional_missing.is_empty() {
        log::info!("ℹ️  以下可选集成未配置（不影响核心功能）：");
        for (var, desc) in optional_missing {
            log::info!("   • {} —— {}", var, desc);
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
    ];

    let sql_plugin = Builder::default()
        .add_migrations("sqlite:flowtime.db", migrations)
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(sql_plugin)
        .setup(|app| {
            // Initialize logger
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();

            log::info!("🚀 Flowtime starting up...");
            validate_environment();

            let pool = app.state::<sqlx::SqlitePool>().inner().clone();
            tauri::async_runtime::spawn(async move {
                api::start_api_server(pool).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // M3: Focus mode
            focus::start_focus_session,
            focus::end_focus_session,
            focus::get_active_focus_session,
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
            // M8: Behavior learning
            learning::get_efficiency_pattern,
            learning::calibrate_estimate,
            learning::get_peak_hours,
            learning::get_calibration_ratio,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}