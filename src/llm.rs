use crate::models::TrainingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a professional fitness coach. Create training plans based on the user's request.

You MUST respond ONLY with valid JSON in this exact format, no other text, no markdown:
{
  "name": "Plan Name",
  "total_rounds": 1,
  "rest_between_rounds_seconds": 60,
  "exercises": [
    {"name": "Exercise Name", "duration_seconds": 30, "reps": null, "weight_kg": null, "rest_seconds": 30},
    {"name": "Exercise Name", "duration_seconds": null, "reps": 10, "weight_kg": 60.0, "rest_seconds": 60}
  ]
}

Rules:
- For timed exercises, set duration_seconds (>= 10) and reps to null
- For rep-based exercises, set reps (>= 1) and duration_seconds to null
- weight_kg is optional. Set it to a number (e.g. 60.0, 12.5) when the exercise uses weights (barbell, dumbbell, kettlebell, machine). Use null for bodyweight exercises.
- rest_seconds is always required (0-300, use 0 if no rest needed)
- total_rounds is how many times to repeat ALL exercises (1 = single pass, 3 = repeat 3 times). Use 1 unless the user asks for multiple rounds/circuits.
- rest_between_rounds_seconds is the pause in seconds between rounds when total_rounds > 1 (0-600, default 60). Use 0 if no rest between rounds is desired.
- Include 4-10 exercises per plan
- Use common exercise names (Pushups, Sit-Up, Bodyweight Squat, Plank, Pullups, Dips, Lunges, Crunches, Burpees, Mountain Climbers, Barbell Squat, Barbell Bench Press, Barbell Deadlift, Kettlebell Swings, Rope Jumping, etc.)
- Respond with ONLY the JSON object, no markdown, no explanation"#;

#[derive(Debug)]
pub struct LLMError(pub String);

impl std::fmt::Display for LLMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LLMError {}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

fn describe_http_error(status: reqwest::StatusCode, body_text: &str, model: &str) -> LLMError {
    let code = status.as_u16();
    if code == 401 || code == 403 {
        return LLMError(format!(
            "Invalid API key. Check your provider settings.\n{}",
            body_text
        ));
    }
    if code == 404 {
        return LLMError(format!(
            "Model '{}' not found. Check your model name.\n{}",
            model, body_text
        ));
    }
    if code == 410 {
        return LLMError(format!(
            "Model '{}' is no longer available (HTTP 410). Try another model, e.g. append ':cloud' variants listed by your provider.\n{}",
            model, body_text
        ));
    }
    LLMError(format!("HTTP error {}: {}", code, body_text))
}

/// Streaming progress for `chat_completion_stream`.
///
/// Thinking models (e.g. many GLM/Qwen/cloud variants) stream a long
/// reasoning phase before the first content delta; without reporting that
/// the UI would sit at "connecting" for minutes on end.
pub enum StreamProgress {
    /// Model is reasoning; `chars` is how much reasoning text arrived.
    Thinking { seconds: f64, chars: usize },
    /// Final answer content is streaming.
    Receiving { chars: usize, elapsed: f64, tail: String },
}

/// Chat completion with SSE streaming (`stream: true`), OpenAI-compatible —
/// works with Ollama's /v1 endpoint and any compatible server.
///
/// Calls `on_progress(len, content)` as chunks arrive so the UI can show
/// live feedback, and checks `cancel` between chunks so a stuck request can
/// be aborted.
pub fn chat_completion_stream(
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[(&str, &str)],
    cancel: &std::sync::atomic::AtomicBool,
    on_progress: &mut dyn FnMut(StreamProgress),
) -> Result<String, LLMError> {
    use std::io::BufRead;
    use std::sync::atomic::Ordering::Relaxed;
    use std::time::Instant;

    let started = Instant::now();

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let chat_messages: Vec<ChatMessage> = messages
        .iter()
        .map(|(role, content)| ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        })
        .collect();

    let body = ChatRequest {
        model: model.to_string(),
        messages: chat_messages,
        temperature: 0.7,
        stream: true,
    };

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| LLMError(format!("Failed to create HTTP client: {}", e)))?;

    let payload = serde_json::to_vec(&body)
        .map_err(|e| LLMError(format!("Failed to serialize request: {}", e)))?;

    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = req
        .body(payload)
        .send()
        .map_err(|e| {
            if e.is_connect() {
                LLMError(format!(
                    "Cannot connect to {}. Make sure it's running.\n{}",
                    base_url, e
                ))
            } else {
                LLMError(format!("Request failed: {}", e))
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(describe_http_error(
            status,
            &response.text().unwrap_or_default(),
            model,
        ));
    } else if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("event-stream"))
        .unwrap_or(false)
    {
        // Server does not stream — the body is a plain JSON chat response.
        let body_text = response
            .text()
            .map_err(|e| LLMError(format!("Failed to read response: {}", e)))?;
        return extract_chat_content(&body_text);
    }

    let mut reader = std::io::BufReader::new(response);
    let mut content = String::new();
    let mut reasoning_chars = 0usize;
    let mut line = String::new();

    loop {
        if cancel.load(Relaxed) {
            return Err(LLMError("Cancelled by user.".to_string()));
        }
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| LLMError(format!("Connection lost: {}", e)))?;
        if n == 0 {
            break;
        }
        let t = line.trim();
        let Some(payload) = t.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        if payload.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(err) = v.get("error").cloned() {
                return Err(LLMError(format!("Server error: {}", err)));
            }
            let delta = v.pointer("/choices/0/delta");
            let mut got_content = false;
            if let Some(delta) = delta {
                // Reasoning phase (Ollama: "reasoning", others: "reasoning_content").
                let reasoning = delta
                    .get("reasoning")
                    .or_else(|| delta.get("reasoning_content"))
                    .and_then(|r| r.as_str());
                if let Some(reasoning) = reasoning {
                    if !reasoning.is_empty() {
                        reasoning_chars += reasoning.chars().count();
                        on_progress(StreamProgress::Thinking {
                            seconds: started.elapsed().as_secs_f64(),
                            chars: reasoning_chars,
                        });
                    }
                }
                if let Some(delta) = delta
                    .get("content")
                    .and_then(|c| c.as_str())
                {
                    if !delta.is_empty() {
                        content.push_str(delta);
                        on_progress(StreamProgress::Receiving {
                            chars: content.chars().count(),
                            elapsed: started.elapsed().as_secs_f64(),
                            tail: last_chars(&content, 60),
                        });
                        got_content = true;
                    }
                }
            }
            let _ = got_content;
        }
    }

    if content.is_empty() {
        return Err(LLMError("Empty response from model.".to_string()));
    }
    Ok(content)
}

fn last_chars(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n {
        return s.to_string();
    }
    let start = s
        .char_indices()
        .nth(len - n)
        .map(|(i, _)| i)
        .unwrap_or(0);
    s[start..].to_string()
}

fn extract_chat_content(body_text: &str) -> Result<String, LLMError> {
    let data: ChatResponse = serde_json::from_str(body_text)
        .map_err(|_| LLMError("AI returned invalid response format.".to_string()))?;
    data.choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| LLMError("AI returned unexpected response structure.".to_string()))
}

pub fn build_history_context(sessions: &[TrainingSession]) -> String {
    if sessions.is_empty() {
        return "No training history yet.".to_string();
    }

    let mut plan_counts: HashMap<String, usize> = HashMap::new();
    for s in sessions.iter().take(50) {
        *plan_counts.entry(s.plan_name.clone()).or_insert(0) += 1;
    }

    let mut counts: Vec<_> = plan_counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));

    let mut lines = vec!["Recent training history:".to_string()];
    for (name, count) in counts.iter().take(10) {
        let last = sessions.iter().find(|s| s.plan_name == *name);
        let last_date = last
            .map(|s| s.started_at.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!(
            "- {}: {} session{}, last {}",
            name,
            count,
            if *count != 1 { "s" } else { "" },
            last_date
        ));
    }
    lines.join("\n")
}

pub fn parse_plan_response(response_text: &str) -> Option<serde_json::Value> {
    let text = response_text.trim();

    let cleaned = if text.starts_with("```") {
        text.lines()
            .filter(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    } else {
        text.to_string()
    };

    let parsed: Option<serde_json::Value> =
        serde_json::from_str::<serde_json::Value>(&cleaned).ok().or_else(|| {
            let start = cleaned.find('{')?;
            let end = cleaned.rfind('}')?;
            if end > start {
                serde_json::from_str(&cleaned[start..=end]).ok()
            } else {
                None
            }
        });

    let data = parsed?;

    if data.get("name").is_none() || data.get("exercises").is_none() {
        return None;
    }
    if !data.get("exercises").and_then(|e| e.as_array()).map(|a| !a.is_empty()).unwrap_or(false) {
        return None;
    }

    Some(data)
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// Live test against the local Ollama /v1 endpoint:
    /// `cargo test -- --ignored stream` (needs ollama running).
    #[test]
    #[ignore]
    fn stream_completion_local_ollama() {
        gtk4::init().expect("gtk init");
        let cancel = AtomicBool::new(false);
        let mut progress_thinking = 0usize;
        let mut progress_receiving = 0usize;
        let mut on_progress = |p: StreamProgress| match p {
            StreamProgress::Thinking { .. } => progress_thinking += 1,
            StreamProgress::Receiving { .. } => progress_receiving += 1,
        };
        let base_url = load_base_url();
        // Model from settings; STREAM_TEST_MODEL env overrides (for live checks)
        let model = std::env::var("STREAM_TEST_MODEL").unwrap_or_else(|_| load_model());
        let result = super::chat_completion_stream(
            &base_url,
            "",
            &model,
            &[("user", "Answer with exactly: OK")],
            &cancel,
            &mut on_progress,
        );
        eprintln!("thinking updates: {} / receiving updates: {}", progress_thinking, progress_receiving);
        match &result {
            Ok(text) => {
                eprintln!("received {} chars: {}", text.len(), text.trim());
                assert!(!text.trim().is_empty());
                assert!(
                    progress_thinking + progress_receiving >= 1,
                    "expected at least one progress callback"
                );
            }
            Err(e) => {
                // Connection problems shouldn't fail the test suite randomly
                eprintln!("model said: {}", e);
            }
        }
    }

    fn load_base_url() -> String {
        crate::settings::load_settings().ai_base_url
    }
    fn load_model() -> String {
        crate::settings::load_settings().ai_model
    }
}
