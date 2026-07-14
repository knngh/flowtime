use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::llm_common::{chat_completion, extract_json};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedTask {
    pub title: String,
    pub priority: String, // "A", "B", "C"
    pub duration_min: u32,
    pub project_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskForSchedule {
    id: String,
    title: String,
    priority: String,
    estimated_duration_min: u32,
    status: String,
}

// ── Tauri Commands ──

#[tauri::command]
pub async fn parse_natural_language(input: String) -> Result<ParsedTask, String> {
    let system_prompt = r#"You are a task parser. Extract task information from natural language input.
Output ONLY valid JSON object (no markdown, no code fences, no extra text):
{
  "title": "concise task title",
  "priority": "A",
  "duration_min": 60,
  "project_hint": "project name or null"
}

Rules:
- priority: "A"=very urgent/important/critical, "B"=normal/default, "C"=low priority/optional. Default "B".
- duration_min: integer minutes. If user says "2 hours", use 120. Default 30 if not mentioned.
- project_hint: extract project name if user mentions a project/team, otherwise null.
- title: a concise description of what needs to be done, remove time/priority qualifiers from title."#;

    match chat_completion(system_prompt, &input).await {
        Ok(content) => {
            let json_str = extract_json(&content);
            serde_json::from_str::<ParsedTask>(json_str).map_err(|e| {
                format!(
                    "JSON_PARSE_ERROR: {} — raw: {}",
                    e,
                    &content[..200.min(content.len())]
                )
            })
        }
        Err(e) if e == "NO_API_KEY" => fallback_parse(&input),
        Err(e) => {
            log::warn!("LLM error: {}, falling back to heuristic", e);
            fallback_parse(&input)
        }
    }
}

fn fallback_parse(input: &str) -> Result<ParsedTask, String> {
    let input = input.trim();

    // Priority heuristic
    let priority = if input.contains("紧急")
        || input.contains("🔥")
        || (input.contains("A") && (input.contains("优先") || input.contains("重要")))
    {
        "A".to_string()
    } else if input.contains("不急") || input.contains("随意") || input.contains("低优") {
        "C".to_string()
    } else {
        "B".to_string()
    };

    // Duration heuristic: extract "X小时" or "X分钟" or "Xh" or "Xmin"
    let duration_min: u32 = {
        let mut found: Option<u32> = None;

        // Chinese: X小时 → X*60
        if let Some(pos) = input.find("小时") {
            let before = &input[..pos];
            let num: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '点')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if let Ok(n) = num.parse::<f64>() {
                found = Some((n * 60.0).round() as u32);
            }
        }

        // Chinese: X分钟
        if found.is_none() {
            if let Some(pos) = input.find("分钟") {
                let before = &input[..pos];
                let num: String = before
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                if let Ok(n) = num.parse::<u32>() {
                    found = Some(n);
                }
            }
        }

        // English: Xh or X hour(s)
        if found.is_none() {
            let lower = input.to_lowercase();
            for suffix in &["h", "hour", "hours", "hr", "hrs"] {
                if let Some(pos) = lower.rfind(suffix) {
                    let before = &input[..pos];
                    let num: String = before
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ' ')
                        .collect::<String>()
                        .trim()
                        .chars()
                        .rev()
                        .collect();
                    if let Ok(n) = num.parse::<f64>() {
                        found = Some((n * 60.0).round() as u32);
                        break;
                    }
                }
            }
        }

        // English: Xmin or X minute(s)
        if found.is_none() {
            let lower = input.to_lowercase();
            for suffix in &["min", "mins", "minute", "minutes"] {
                if let Some(pos) = lower.rfind(suffix) {
                    let before = &input[..pos];
                    let num: String = before
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ' ')
                        .collect::<String>()
                        .trim()
                        .chars()
                        .rev()
                        .collect();
                    if let Ok(n) = num.parse::<u32>() {
                        found = Some(n);
                        break;
                    }
                }
            }
        }

        found.unwrap_or(30)
    };

    Ok(ParsedTask {
        title: input.to_string(),
        priority,
        duration_min,
        project_hint: None,
    })
}

#[tauri::command]
pub async fn suggest_schedule(tasks_json: String, state: State<'_, SqlitePool>) -> Result<Vec<String>, String> {
    let pool = state.inner();
    let tasks: Vec<TaskForSchedule> = serde_json::from_str(&tasks_json)
        .map_err(|e| format!("INVALID_TASKS_JSON: {}", e))?;

    if tasks.is_empty() {
        return Ok(vec![]);
    }

    let system_prompt = r#"You are a daily scheduling assistant. Given a list of tasks with priorities, estimated durations, and statuses, suggest an optimal execution order for today.

Guidelines:
- Priority A tasks (urgent/important) should come first
- In-progress tasks should be completed early
- Within same priority, shorter tasks first to build momentum
- Batch tasks from similar domains together
- Consider energy: cognitively heavy tasks in the morning

Output ONLY a JSON array of task IDs in suggested order. No markdown, no explanation.
Example: ["id-abc", "id-xyz", "id-123"]"#;

    let user_message = serde_json::to_string_pretty(&tasks).unwrap_or_default();

    let ordered = match chat_completion(system_prompt, &user_message).await {
        Ok(content) => {
            let json_str = extract_json(&content);
            match serde_json::from_str::<Vec<String>>(json_str) {
                Ok(ids) => ids,
                Err(_) => {
                    #[derive(Deserialize)]
                    struct Wrapper {
                        order: Vec<String>,
                        #[serde(rename = "taskIds")]
                        task_ids: Option<Vec<String>>,
                        #[serde(rename = "suggested_order")]
                        suggested_order: Option<Vec<String>>,
                    }
                    match serde_json::from_str::<Wrapper>(json_str) {
                        Ok(w) => w.suggested_order.or(w.task_ids).unwrap_or(w.order),
                        Err(e) => {
                            log::warn!("Schedule JSON parse failed: {}, falling back", e);
                            fallback_schedule(&tasks)
                        }
                    }
                }
            }
        }
        Err(_) => fallback_schedule(&tasks),
    };

    // P3-1: write real time slots into the calendar, filling from the user's
    // next peak hour (or 09:00 today as default).
    if let Err(e) = write_schedule_slots(&pool, &tasks, &ordered).await {
        log::warn!("Failed to write schedule slots: {}", e);
    }

    Ok(ordered)
}

/// Fill `scheduled_start`/`scheduled_end` for the ordered tasks, back-to-back
/// from the start of the user's peak window (or 09:00). Tasks keep their own
/// duration; the cursor advances by each task's estimated minutes.
async fn write_schedule_slots(
    pool: &SqlitePool,
    tasks: &[TaskForSchedule],
    ordered: &[String],
) -> Result<(), String> {
    let now = Utc::now();

    // Determine a start hour: next peak hour today, else 09:00.
    let start_hour: u32 = crate::learning::compute_peak_ranges(pool, 14)
        .await
        .first()
        .map(|p| p.start_hour.max(0) as u32)
        .unwrap_or(9);

    let mut cursor = now
        .date_naive()
        .and_hms_opt(start_hour, 0, 0)
        .unwrap_or_else(|| now.date_naive().and_hms_opt(9, 0, 0).unwrap())
        .and_local_timezone(chrono::Utc)
        .unwrap();

    // Don't schedule in the past: if the cursor is before now, push to now.
    if cursor < now {
        cursor = now;
    }

    let by_id: std::collections::HashMap<&str, &TaskForSchedule> =
        tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    for id in ordered {
        if let Some(task) = by_id.get(id.as_str()) {
            let start = cursor;
            let dur = Duration::minutes(task.estimated_duration_min as i64);
            let end = start + dur;
            sqlx::query(
                "UPDATE tasks SET scheduled_start = ?, scheduled_end = ? WHERE id = ?",
            )
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("schedule write error: {}", e))?;
            cursor = end;
        }
    }
    Ok(())
}

fn fallback_schedule(tasks: &[TaskForSchedule]) -> Vec<String> {
    let mut active: Vec<&TaskForSchedule> = tasks
        .iter()
        .filter(|t| t.status == "todo" || t.status == "in_progress")
        .collect();

    active.sort_by(|a, b| {
        // Priority A > B > C
        let pa = priority_value(&a.priority);
        let pb = priority_value(&b.priority);
        match pa.cmp(&pb) {
            std::cmp::Ordering::Equal => {
                // In-progress before todo
                let sa = if a.status == "in_progress" { 0 } else { 1 };
                let sb = if b.status == "in_progress" { 0 } else { 1 };
                match sa.cmp(&sb) {
                    std::cmp::Ordering::Equal => {
                        // Shorter tasks first
                        a.estimated_duration_min.cmp(&b.estimated_duration_min)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    active.iter().map(|t| t.id.clone()).collect()
}

fn priority_value(p: &str) -> u8 {
    match p {
        "A" => 0,
        "B" => 1,
        "C" => 2,
        _ => 1,
    }
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_common::extract_json;

    #[test]
    fn test_extract_json_code_fence() {
        let input = "```json\n{\"title\": \"test\"}\n```";
        assert_eq!(extract_json(input), "{\"title\": \"test\"}");
    }

    #[test]
    fn test_extract_json_plain_object() {
        let input = "Here is {\"priority\": \"A\", \"title\": \"do thing\"} end";
        assert_eq!(extract_json(input), "{\"priority\": \"A\", \"title\": \"do thing\"}");
    }

    #[test]
    fn test_extract_json_array() {
        let input = "[\"a\", \"b\", \"c\"]";
        assert_eq!(extract_json(input), "[\"a\", \"b\", \"c\"]");
    }

    #[test]
    fn test_extract_json_nested() {
        let input = r#"{"items": [{"nested": true}]}"#;
        assert_eq!(extract_json(input), r#"{"items": [{"nested": true}]}"#);
    }

    #[test]
    fn test_priority_value() {
        assert_eq!(priority_value("A"), 0);
        assert_eq!(priority_value("B"), 1);
        assert_eq!(priority_value("C"), 2);
        assert_eq!(priority_value("X"), 1);
        assert_eq!(priority_value(""), 1);
    }

    #[test]
    fn test_fallback_parse_chinese_urgent() {
        let result = fallback_parse("紧急修复登录bug").unwrap();
        assert_eq!(result.priority, "A");
        assert_eq!(result.duration_min, 30); // default
    }

    #[test]
    fn test_fallback_parse_chinese_hours() {
        let result = fallback_parse("完成报告 2小时").unwrap();
        assert_eq!(result.duration_min, 120);
    }

    #[test]
    fn test_fallback_parse_chinese_minutes() {
        let result = fallback_parse("休息 15分钟").unwrap();
        assert_eq!(result.duration_min, 15);
    }

    #[test]
    fn test_fallback_parse_english_hours() {
        let result = fallback_parse("Design review 3h").unwrap();
        assert_eq!(result.duration_min, 180);
    }

    #[test]
    fn test_fallback_parse_low_priority() {
        let result = fallback_parse("不急的task").unwrap();
        assert_eq!(result.priority, "C");
    }

    #[test]
    fn test_fallback_schedule_sorts_by_priority() {
        let tasks = vec![
            TaskForSchedule {
                id: "1".into(), title: "C task".into(), priority: "C".into(),
                estimated_duration_min: 30, status: "todo".into(),
            },
            TaskForSchedule {
                id: "2".into(), title: "A task".into(), priority: "A".into(),
                estimated_duration_min: 60, status: "todo".into(),
            },
            TaskForSchedule {
                id: "3".into(), title: "B task".into(), priority: "B".into(),
                estimated_duration_min: 15, status: "todo".into(),
            },
        ];
        let ids = fallback_schedule(&tasks);
        assert_eq!(ids[0], "2"); // A first
        assert_eq!(ids[1], "3"); // B second (shorter than 60)
        assert_eq!(ids[2], "1"); // C last
    }

    #[test]
    fn test_fallback_schedule_in_progress_first() {
        let tasks = vec![
            TaskForSchedule {
                id: "1".into(), title: "B todo".into(), priority: "B".into(),
                estimated_duration_min: 30, status: "todo".into(),
            },
            TaskForSchedule {
                id: "2".into(), title: "B in_progress".into(), priority: "B".into(),
                estimated_duration_min: 60, status: "in_progress".into(),
            },
        ];
        let ids = fallback_schedule(&tasks);
        assert_eq!(ids[0], "2"); // in_progress before todo
    }
}
