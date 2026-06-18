use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

fn get_llm_config() -> (String, String, String) {
    // Check for Ollama first (local LLM takes priority if configured)
    let ollama_base = std::env::var("OLLAMA_API_BASE")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_default();

    if !ollama_base.is_empty() {
        let api_base = ollama_base.trim_end_matches('/').to_string();
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "qwen2.5:7b".to_string());
        let api_key = "ollama".to_string();
        return (api_base, api_key, model);
    }

    let api_base =
        std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let model =
        std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    (api_base, api_key, model)
}

async fn chat_completion(system_prompt: &str, user_message: &str) -> Result<String, String> {
    let (api_base, api_key, model) = get_llm_config();
    let is_ollama = api_key == "ollama";

    // Ollama works without OpenAI API key; OpenAI requires key
    if api_key.is_empty() || (api_key != "ollama" && api_key.is_empty()) {
        return Err("NO_API_KEY".to_string());
    }

    if !is_ollama && api_key.is_empty() {
        return Err("NO_API_KEY".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("CLIENT_ERROR: {}", e))?;

    let request = ChatRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            },
        ],
        temperature: 0.1,
    };

    let response = client
        .post(format!(
            "{}/chat/completions",
            api_base.trim_end_matches('/')
        ))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("NETWORK_ERROR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "API_ERROR: HTTP {} — {}",
            status,
            &body[..200.min(body.len())]
        ));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("PARSE_ERROR: {}", e))?;

    chat_response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "EMPTY_RESPONSE".to_string())
}

fn extract_json(content: &str) -> &str {
    let content = content.trim();

    // Try to find JSON in ```json ... ```
    if let Some(start) = content.find("```json") {
        let after = &content[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    // Try to find JSON in ``` ... ```
    if let Some(start) = content.find("```") {
        let after = &content[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }

    // Find outermost JSON object/array by brace/bracket matching
    if let Some(start) = content.find(|c| c == '{' || c == '[') {
        let rest = &content[start..];
        let open: char = rest.chars().next().unwrap();
        let close: char = if open == '{' { '}' } else { ']' };
        let mut depth = 0;
        for (i, ch) in rest.char_indices() {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return &rest[..=i];
                }
            }
        }
    }
    content
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
pub async fn suggest_schedule(tasks_json: String) -> Result<Vec<String>, String> {
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

    match chat_completion(system_prompt, &user_message).await {
        Ok(content) => {
            let json_str = extract_json(&content);
            // Try direct array parse
            match serde_json::from_str::<Vec<String>>(json_str) {
                Ok(ids) => Ok(ids),
                Err(_) => {
                    // Try wrapped in object
                    #[derive(Deserialize)]
                    struct Wrapper {
                        order: Vec<String>,
                        #[serde(rename = "taskIds")]
                        task_ids: Option<Vec<String>>,
                        #[serde(rename = "suggested_order")]
                        suggested_order: Option<Vec<String>>,
                    }
                    match serde_json::from_str::<Wrapper>(json_str) {
                        Ok(w) => {
                            let ids = w
                                .suggested_order
                                .or(w.task_ids)
                                .unwrap_or(w.order);
                            Ok(ids)
                        }
                        Err(e) => Err(format!(
                            "JSON_PARSE_ERROR: {} — raw: {}",
                            e,
                            &content[..200.min(content.len())]
                        )),
                    }
                }
            }
        }
        Err(_) => Ok(fallback_schedule(&tasks)),
    }
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