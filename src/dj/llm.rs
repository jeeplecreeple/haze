//! Bring-your-own-key LLM providers. Each takes a system prompt and a user prompt and returns text.
//!
//! - OpenAI and anything OpenAI-compatible (OpenRouter, Groq, Mistral, DeepSeek, xAI, Together,
//!   Ollama, LM Studio...) through `/chat/completions`.
//! - Anthropic Claude through `/messages`.
//! - Google Gemini through `:generateContent`.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use crate::config::{DjConfig, LlmProvider};

pub async fn complete(config: &DjConfig, system: &str, user: &str) -> Result<String> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
    let key = config.api_key();
    match config.provider {
        LlmProvider::OpenAI => openai(&http, config, key, system, user).await,
        LlmProvider::Anthropic => {
            anthropic(&http, config, key.ok_or_else(|| missing_key(config))?, system, user).await
        }
        LlmProvider::Gemini => gemini(&http, config, key.ok_or_else(|| missing_key(config))?, system, user).await,
    }
}

fn missing_key(config: &DjConfig) -> anyhow::Error {
    let var = crate::config::non_empty(&config.api_key_env).unwrap_or(config.provider.default_key_env());
    anyhow!("No API key for the DJ. Set dj.api_key in settings, or the {var} environment variable.")
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value> {
    let resp = req.send().await?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| body.to_string());
        bail!("LLM request failed ({status}): {msg}");
    }
    Ok(body)
}

async fn openai(
    http: &reqwest::Client,
    config: &DjConfig,
    key: Option<String>,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/chat/completions", config.base_url());
    let mut body = json!({
        "model": config.model(),
        "temperature": config.temperature,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "response_format": {"type": "json_object"},
    });
    let build = |body: &Value| {
        let mut req = http.post(&url).json(body);
        if let Some(key) = &key {
            req = req.bearer_auth(key);
        }
        req
    };
    let resp = match send(build(&body)).await {
        Ok(v) => v,
        // Some compatible servers don't support JSON mode or custom temperature; retry plainly.
        Err(first) => {
            let obj = body.as_object_mut().expect("object");
            obj.remove("response_format");
            obj.remove("temperature");
            send(build(&body)).await.map_err(|_| first)?
        }
    };
    resp.pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("LLM returned no text"))
}

/// Models that accept Anthropic's server-side refusal fallback.
fn supports_fallbacks(model: &str) -> bool {
    ["claude-fable-5-1", "claude-opus-5-5", "claude-opus-5", "claude-sonnet-5-5"].contains(&model)
}

async fn anthropic(
    http: &reqwest::Client,
    config: &DjConfig,
    key: String,
    system: &str,
    user: &str,
) -> Result<String> {
    let model = config.model();
    let official = config.base_url() == LlmProvider::Anthropic.default_base_url();
    // Current Claude models choose their own sampling, so no temperature is sent.
    let mut body = json!({
        "model": model,
        "max_tokens": 8000,
        "system": system,
        "messages": [{"role": "user", "content": user}],
    });
    let mut req = http
        .post(format!("{}/messages", config.base_url()))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01");
    if official && supports_fallbacks(&model) {
        // If a safety classifier declines, let the API retry on a suitable model.
        body["fallbacks"] = json!("default");
        req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
    }
    let resp = send(req.json(&body)).await?;
    if resp.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        bail!("Claude declined this request");
    }
    let text: String = resp
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    if text.is_empty() {
        bail!("Claude returned no text");
    }
    Ok(text)
}

async fn gemini(
    http: &reqwest::Client,
    config: &DjConfig,
    key: String,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/models/{}:generateContent", config.base_url(), config.model());
    let body = json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": user}]}],
        "generationConfig": {
            "temperature": config.temperature,
            "responseMimeType": "application/json",
        },
    });
    let resp = send(http.post(url).header("x-goog-api-key", key).json(&body)).await?;
    let text: String = resp
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect();
    if text.is_empty() {
        bail!("Gemini returned no text");
    }
    Ok(text)
}
