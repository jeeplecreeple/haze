//! Bring-your-own-key LLM providers. Each takes a system prompt and a user prompt and returns text.
//!
//! - OpenAI and anything OpenAI-compatible (Mistral, Groq, OpenRouter, DeepSeek, xAI, Cerebras,
//!   Ollama, LM Studio...) through `/chat/completions`.
//! - Anthropic Claude through `/messages`.
//! - Google Gemini through `:generateContent`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use crate::config::{DjConfig, LlmProvider};

pub async fn complete(config: &DjConfig, system: &str, user: &str) -> Result<String> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
    let key = config.api_key();
    if key.is_none() && config.provider.needs_key() {
        return Err(missing_key(config));
    }
    match config.provider {
        LlmProvider::Anthropic => anthropic(&http, config, key.unwrap_or_default(), system, user).await,
        LlmProvider::Gemini => gemini(&http, config, key.unwrap_or_default(), system, user).await,
        _ => openai(&http, config, key, system, user).await,
    }
}

fn missing_key(config: &DjConfig) -> anyhow::Error {
    let var = crate::config::non_empty(&config.api_key_env).unwrap_or(config.provider.default_key_env());
    anyhow!("No API key for the DJ. Set dj.api_key in settings, or the {var} environment variable.")
}

/// Sends the request, waiting and retrying a couple of times while the provider is overloaded
/// (Gemini's "model is overloaded", 502/503/529s).
async fn send(req: reqwest::RequestBuilder) -> Result<Value> {
    let mut wait = Duration::from_secs(2);
    for attempt in 0..3 {
        let this = req.try_clone().ok_or_else(|| anyhow!("request can't be resent"))?;
        match req_send(this).await {
            Ok(result) => return result,
            Err(Busy(msg)) if attempt < 2 => {
                log::info!("DJ provider is busy ({msg}); retrying in {}s", wait.as_secs());
                tokio::time::sleep(wait).await;
                wait *= 3;
            }
            Err(Busy(msg)) => bail!(
                "The AI provider is too busy right now ({msg}). Try again soon, or pick another provider in Settings > DJ."
            ),
        }
    }
    unreachable!()
}

struct Busy(String);

async fn req_send(req: reqwest::RequestBuilder) -> std::result::Result<Result<Value>, Busy> {
    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(e) => return Ok(Err(e.into())),
    };
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(Ok(body));
    }
    let msg = body
        .pointer("/error/message")
        .or_else(|| body.get("message"))
        .or_else(|| body.get("detail"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    if matches!(status.as_u16(), 502 | 503 | 529) {
        return Err(Busy(msg));
    }
    Ok(Err(anyhow!("LLM request failed ({status}): {msg}")))
}

/// Model names go stale (providers retire them), so when the configured one is refused we ask the
/// server which models it has and use one of those. Remembered per base URL for this run.
static MODEL_SUBSTITUTES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn substitute(base: &str) -> Option<String> {
    MODEL_SUBSTITUTES.lock().ok()?.as_ref()?.get(base).cloned()
}

fn looks_like_bad_model(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("model") && ["404", "not found", "not exist", "invalid", "decommissioned", "unknown"].iter().any(|w| msg.contains(w))
}

async fn pick_model(http: &reqwest::Client, base: &str, key: Option<&str>, wanted: &str) -> Option<String> {
    let mut req = http.get(format!("{base}/models"));
    if let Some(key) = key {
        req = req.bearer_auth(key);
    }
    let list = req_send(req).await.ok()?.ok()?;
    let ids: Vec<&str> = list
        .get("data")
        .or_else(|| list.get("models"))
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|m| m.get("id").or_else(|| m.get("name")).and_then(Value::as_str))
        .filter(|id| {
            let id = id.to_lowercase();
            !["embed", "whisper", "tts", "audio", "image", "vision", "moderation", "guard", "ocr", "transcribe", "realtime"]
                .iter()
                .any(|w| id.contains(w))
        })
        .collect();
    // Prefer something from the same family as what was asked for, then anything chat-like.
    let family = wanted.split(['-', '/', ':']).next().unwrap_or(wanted).to_lowercase();
    ids.iter()
        .find(|id| id.to_lowercase().contains(&family))
        .or_else(|| ids.first())
        .map(|id| id.to_string())
}

async fn openai(
    http: &reqwest::Client,
    config: &DjConfig,
    key: Option<String>,
    system: &str,
    user: &str,
) -> Result<String> {
    let base = config.base_url();
    let url = format!("{base}/chat/completions");
    let model = substitute(&base).unwrap_or_else(|| config.model());
    let mut body = json!({
        "model": model,
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
        Err(first) if looks_like_bad_model(&first) => {
            let Some(other) = pick_model(http, &base, key.as_deref(), &model).await.filter(|m| *m != model) else {
                return Err(first);
            };
            log::warn!("model {model} was refused ({first}); using {other} instead");
            body["model"] = json!(other);
            let resp = send(build(&body)).await.map_err(|_| first)?;
            if let Ok(mut subs) = MODEL_SUBSTITUTES.lock() {
                subs.get_or_insert_with(HashMap::new).insert(base.clone(), other);
            }
            resp
        }
        // Some compatible servers don't support JSON mode or custom temperature; retry plainly.
        Err(first) => {
            let obj = body.as_object_mut().expect("object");
            obj.remove("response_format");
            obj.remove("temperature");
            send(build(&body)).await.map_err(|second| {
                log::warn!("plain retry failed too: {second}");
                first
            })?
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
