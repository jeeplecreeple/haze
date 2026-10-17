//! Text-to-speech for spoken DJ intros. Each call returns once the speech has finished.

use std::io::Cursor;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use tokio::process::Command;

use crate::config::{DjConfig, LlmProvider, TtsEngine, non_empty};

pub async fn speak(config: &DjConfig, text: &str) -> Result<()> {
    match config.tts.engine {
        TtsEngine::System => system(config, text).await,
        TtsEngine::OpenAI => openai(config, text).await,
    }
}

async fn system(config: &DjConfig, text: &str) -> Result<()> {
    let voice = non_empty(&config.tts.voice);
    let status = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("say");
        if let Some(v) = voice {
            cmd.args(["-v", v]);
        }
        cmd.arg(text).status().await.context("running `say`")?
    } else if cfg!(target_os = "windows") {
        // Text goes through an environment variable so it is never parsed as script.
        let script = "Add-Type -AssemblyName System.Speech; \
            $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
            if ($env:HAZE_VOICE) { $s.SelectVoice($env:HAZE_VOICE) }; \
            $s.Speak($env:HAZE_TTS)";
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("HAZE_TTS", text)
            .env("HAZE_VOICE", voice.unwrap_or_default())
            .status()
            .await
            .context("running PowerShell speech")?
    } else {
        let mut last_err = None;
        let mut found = None;
        for program in ["espeak-ng", "espeak", "spd-say"] {
            let mut cmd = Command::new(program);
            match (program, voice) {
                ("spd-say", Some(v)) => {
                    cmd.args(["-w", "-y", v]);
                }
                ("spd-say", None) => {
                    cmd.arg("-w");
                }
                (_, Some(v)) => {
                    cmd.args(["-v", v]);
                }
                _ => {}
            }
            match cmd.arg(text).status().await {
                Ok(status) => {
                    found = Some(status);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        found.ok_or_else(|| {
            anyhow!(
                "No speech program found (install espeak-ng or speech-dispatcher), or switch the DJ voice to OpenAI-compatible TTS: {}",
                last_err.map(|e| e.to_string()).unwrap_or_default()
            )
        })?
    };
    if !status.success() {
        bail!("speech program exited with {status}");
    }
    Ok(())
}

async fn openai(config: &DjConfig, text: &str) -> Result<()> {
    let tts = &config.tts;
    let base = match non_empty(&tts.base_url) {
        Some(url) => url.trim_end_matches('/').to_string(),
        None if config.provider == LlmProvider::OpenAI => config.base_url(),
        None => LlmProvider::OpenAI.default_base_url().to_string(),
    };
    let key = match non_empty(&tts.api_key_env) {
        Some(var) => std::env::var(var).ok(),
        None if config.provider == LlmProvider::OpenAI => config.api_key(),
        None => std::env::var("OPENAI_API_KEY").ok(),
    };
    let body = json!({
        "model": non_empty(&tts.model).unwrap_or("gpt-4o-mini-tts"),
        "voice": non_empty(&tts.voice).unwrap_or("nova"),
        "input": text,
        "response_format": "mp3",
    });
    let mut req = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?
        .post(format!("{base}/audio/speech"))
        .json(&body);
    if let Some(key) = key {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        bail!("TTS request failed ({status}): {}", resp.text().await.unwrap_or_default());
    }
    let audio = resp.bytes().await?.to_vec();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let stream = rodio::OutputStreamBuilder::open_default_stream().context("opening audio output")?;
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.append(rodio::Decoder::new(Cursor::new(audio)).context("decoding speech audio")?);
        sink.sleep_until_end();
        Ok(())
    })
    .await?
}
