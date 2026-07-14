use serde::{Deserialize, Serialize};

// ── Unified external task ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalTask {
    pub external_id: String,
    pub title: String,
    pub source: String, // "github" | "linear" | "feishu"
    pub url: Option<String>,
    pub priority_hint: String, // "A" | "B" | "C"
}

// ── GitHub ──

#[derive(Debug, Deserialize)]
struct GitHubIssue {
    number: u64,
    title: String,
    html_url: String,
    state: String,
    labels: Vec<GitHubLabel>,
}

#[derive(Debug, Deserialize)]
struct GitHubLabel {
    name: String,
}

fn github_priority(labels: &[GitHubLabel]) -> &str {
    let names: Vec<&str> = labels.iter().map(|l| l.name.as_str()).collect();
    if names.iter().any(|n| n.contains("urgent") || n.contains("critical") || n.contains("P0")) {
        "A"
    } else if names.iter().any(|n| n.contains("low") || n.contains("P2") || n.contains("nice-to-have")) {
        "C"
    } else {
        "B"
    }
}

#[tauri::command]
pub async fn fetch_github_issues() -> Result<Vec<ExternalTask>, String> {
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return Err("GITHUB_TOKEN 环境变量未设置".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("CLIENT_ERROR: {}", e))?;

    let url = "https://api.github.com/issues?filter=assigned&state=open&per_page=50";
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "flowtime-app")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("NETWORK_ERROR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error: HTTP {} — {}", status, &body[..200.min(body.len())]));
    }

    let issues: Vec<GitHubIssue> = response
        .json()
        .await
        .map_err(|e| format!("PARSE_ERROR: {}", e))?;

    let tasks: Vec<ExternalTask> = issues
        .into_iter()
        .filter(|i| i.state == "open")
        .map(|i| ExternalTask {
            external_id: format!("gh-{}", i.number),
            title: i.title,
            source: "github".to_string(),
            url: Some(i.html_url),
            priority_hint: github_priority(&i.labels).to_string(),
        })
        .collect();

    Ok(tasks)
}

// ── Linear ──

#[derive(Debug, Deserialize)]
struct LinearGraphQLResponse {
    data: Option<LinearData>,
    errors: Option<Vec<LinearError>>,
}

#[derive(Debug, Deserialize)]
struct LinearData {
    issues: LinearIssues,
}

#[derive(Debug, Deserialize)]
struct LinearIssues {
    nodes: Vec<LinearIssue>,
}

#[derive(Debug, Deserialize)]
struct LinearIssue {
    id: String,
    identifier: String,
    title: String,
    url: String,
    priority_label: Option<String>,
    state: LinearState,
}

#[derive(Debug, Deserialize)]
struct LinearState {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinearError {
    message: String,
}

fn linear_priority(label: Option<&str>) -> &str {
    match label {
        Some("Urgent") | Some("High") => "A",
        Some("Low") => "C",
        _ => "B",
    }
}

#[tauri::command]
pub async fn fetch_linear_issues() -> Result<Vec<ExternalTask>, String> {
    let api_key = std::env::var("LINEAR_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err("LINEAR_API_KEY 环境变量未设置".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("CLIENT_ERROR: {}", e))?;

    let query = r#"
        query {
            issues(filter: { assignee: { isMe: { eq: true } }, state: { name: { in: ["Todo", "In Progress"] } } }, first: 50) {
                nodes {
                    id
                    identifier
                    title
                    url
                    priorityLabel
                    state { name }
                }
            }
        }
    "#;

    let body = serde_json::json!({ "query": query });

    let response = client
        .post("https://api.linear.app/graphql")
        .header("Authorization", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("NETWORK_ERROR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Linear API error: HTTP {} — {}", status, &body[..200.min(body.len())]));
    }

    let result: LinearGraphQLResponse = response
        .json()
        .await
        .map_err(|e| format!("PARSE_ERROR: {}", e))?;

    if let Some(errors) = result.errors {
        if !errors.is_empty() {
            return Err(format!("Linear GraphQL error: {}", errors[0].message));
        }
    }

    let issues = result
        .data
        .map(|d| d.issues.nodes)
        .unwrap_or_default();

    let tasks: Vec<ExternalTask> = issues
        .into_iter()
        .map(|i| ExternalTask {
            external_id: format!("linear-{}", i.identifier),
            title: i.title,
            source: "linear".to_string(),
            url: Some(i.url),
            priority_hint: linear_priority(i.priority_label.as_deref()).to_string(),
        })
        .collect();

    Ok(tasks)
}

// ── 飞书日历 ──

#[derive(Debug, Deserialize)]
struct FeishuTokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeishuCalendarResponse {
    code: i32,
    msg: String,
    data: Option<FeishuCalendarData>,
}

#[derive(Debug, Deserialize)]
struct FeishuCalendarData {
    items: Option<Vec<FeishuCalendarEvent>>,
}

#[derive(Debug, Deserialize)]
struct FeishuCalendarEvent {
    event_id: String,
    summary: Option<String>,
    start_time: Option<FeishuTimeInfo>,
    end_time: Option<FeishuTimeInfo>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeishuTimeInfo {
    timestamp: Option<String>,
    date: Option<String>,
}

async fn get_feishu_tenant_token() -> Result<String, String> {
    let app_id = std::env::var("FEISHU_APP_ID").unwrap_or_default();
    let app_secret = std::env::var("FEISHU_APP_SECRET").unwrap_or_default();

    if app_id.is_empty() || app_secret.is_empty() {
        return Err("FEISHU_APP_ID 或 FEISHU_APP_SECRET 环境变量未设置".to_string());
    }

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "app_id": app_id,
        "app_secret": app_secret,
    });

    let response = client
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("飞书认证失败: {}", e))?;

    let result: FeishuTokenResponse = response
        .json()
        .await
        .map_err(|e| format!("飞书认证响应解析失败: {}", e))?;

    if result.code != 0 {
        return Err(format!("飞书认证失败: {} — {}", result.code, result.msg));
    }

    result
        .tenant_access_token
        .ok_or_else(|| "飞书认证未返回 token".to_string())
}

#[tauri::command]
pub async fn fetch_feishu_events() -> Result<Vec<ExternalTask>, String> {
    let token = get_feishu_tenant_token().await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("CLIENT_ERROR: {}", e))?;

    // Get today's date range
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let start = format!("{}T00:00:00+08:00", today);
    let end = format!("{}T23:59:59+08:00", today);

    let response = client
        .get("https://open.feishu.cn/open-apis/calendar/v4/calendars/primary/events")
        .header("Authorization", format!("Bearer {}", token))
        .query(&[
            ("start_time", start.as_str()),
            ("end_time", end.as_str()),
            ("page_size", "50"),
        ])
        .send()
        .await
        .map_err(|e| format!("飞书 API 请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("飞书 API error: HTTP {} — {}", status, &body[..200.min(body.len())]));
    }

    let result: FeishuCalendarResponse = response
        .json()
        .await
        .map_err(|e| format!("飞书日历响应解析失败: {}", e))?;

    if result.code != 0 {
        return Err(format!("飞书日历查询失败: {} — {}", result.code, result.msg));
    }

    let events = result
        .data
        .and_then(|d| d.items)
        .unwrap_or_default();

    let tasks: Vec<ExternalTask> = events
        .into_iter()
        .map(|e| {
            let _time_str = if let Some(st) = &e.start_time {
                if let Some(ts) = &st.timestamp {
                    ts.clone()
                } else if let Some(d) = &st.date {
                    format!("{} 全天", d)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let desc = e.description.unwrap_or_default();
            let priority = if desc.contains("重要") || desc.contains("紧急") {
                "A"
            } else {
                "B"
            };

            ExternalTask {
                external_id: format!("feishu-{}", &e.event_id[..12.min(e.event_id.len())]),
                title: e.summary.unwrap_or_else(|| "无标题事件".to_string()),
                source: "feishu".to_string(),
                url: None,
                priority_hint: priority.to_string(),
            }
        })
        .collect();

    Ok(tasks)
}

// ── Import external tasks (returns list, does NOT auto-create) ──

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub tasks: Vec<ExternalTask>,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportError {
    pub source: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportSource {
    pub source: String, // "github" | "linear" | "feishu"
}

#[tauri::command]
pub async fn import_external_tasks(
    sources: Vec<String>,
) -> Result<ImportResult, String> {
    if sources.is_empty() {
        return Err("请指定至少一个外部源".to_string());
    }

    let mut all_tasks: Vec<ExternalTask> = Vec::new();
    let mut errors: Vec<ImportError> = Vec::new();

    for source in &sources {
        let result = match source.as_str() {
            "github" => fetch_github_issues().await,
            "linear" => fetch_linear_issues().await,
            "feishu" => fetch_feishu_events().await,
            _ => Err(format!("未知的外部源: {}", source)),
        };

        match result {
            Ok(mut tasks) => {
                let count = tasks.len();
                all_tasks.append(&mut tasks);
                log::info!("[import] {} 导入成功，获取 {} 个任务", source, count);
            }
            Err(e) => {
                log::warn!("[import] {} 导入失败: {}", source, e);
                errors.push(ImportError {
                    source: source.clone(),
                    message: e,
                });
            }
        }
    }

    Ok(ImportResult {
        tasks: all_tasks,
        errors,
    })
}

// ── Unit Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_priority_urgent() {
        let labels = vec![GitHubLabel { name: "P0".to_string() }];
        assert_eq!(github_priority(&labels), "A");
        let labels = vec![GitHubLabel { name: "critical-bug".to_string() }];
        assert_eq!(github_priority(&labels), "A");
    }

    #[test]
    fn test_github_priority_low() {
        let labels = vec![GitHubLabel { name: "nice-to-have".to_string() }];
        assert_eq!(github_priority(&labels), "C");
        let labels = vec![GitHubLabel { name: "P2".to_string() }];
        assert_eq!(github_priority(&labels), "C");
    }

    #[test]
    fn test_github_priority_default() {
        let labels = vec![GitHubLabel { name: "bug".to_string() }];
        assert_eq!(github_priority(&labels), "B");
    }

    #[test]
    fn test_linear_priority() {
        assert_eq!(linear_priority(Some("Urgent")), "A");
        assert_eq!(linear_priority(Some("High")), "A");
        assert_eq!(linear_priority(Some("Low")), "C");
        assert_eq!(linear_priority(Some("Medium")), "B");
        assert_eq!(linear_priority(None), "B");
    }
}