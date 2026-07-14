use serde::{Deserialize, Serialize};

// ── Shared chat-completion types (used by llm.rs and auto_reply.rs) ──

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

/// Resolve the LLM endpoint configuration.
/// Ollama (local model) takes priority when `OLLAMA_API_BASE`/`OLLAMA_HOST` is set.
pub fn get_llm_config() -> (String, String, String) {
    let ollama_base = std::env::var("OLLAMA_API_BASE")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_default();
    if !ollama_base.is_empty() {
        let api_base = ollama_base.trim_end_matches('/').to_string();
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());
        return (api_base, "ollama".to_string(), model);
    }

    let api_base =
        std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    (api_base, api_key, model)
}

/// Call an OpenAI-compatible chat completion endpoint.
/// Ollama is supported transparently (its config sets `api_key = "ollama"`).
pub async fn chat_completion(system_prompt: &str, user_message: &str) -> Result<String, String> {
    let (api_base, api_key, model) = get_llm_config();

    // The only failure mode is "no provider configured": OpenAI needs a real key,
    // Ollama sets api_key="ollama" (non-empty). So a single empty-check suffices.
    if api_key.is_empty() {
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
        .post(format!("{}/chat/completions", api_base.trim_end_matches('/')))
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

/// Extract the first JSON object/array from an LLM response, tolerating
/// ```json fences and surrounding prose.
pub fn extract_json(content: &str) -> &str {
    let content = content.trim();

    if let Some(start) = content.find("```json") {
        let after = &content[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = content.find("```") {
        let after = &content[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }

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
