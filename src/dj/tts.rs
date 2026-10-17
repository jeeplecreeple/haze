//! Text-to-speech for spoken DJ intros. Each call returns once the speech has finished.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::{self, DjConfig, LlmProvider, TtsEngine, non_empty};

/// The error when Linux has no speech program. The app then stops trying until the voice engine changes.
pub const NO_SYSTEM_VOICE: &str = "There's no speech program on this computer, so the DJ talks on screen for now. \
    Install espeak-ng to hear it, or choose Piper in Settings > DJ for a more natural voice.";

/// The error when Piper isn't installed. Handled like `NO_SYSTEM_VOICE`.
pub const NO_PIPER: &str = "Piper isn't installed, so the DJ talks on screen for now. \
    Install it (pipx install piper-tts, or piper-tts-bin from the AUR on Arch), then choose Piper again in Settings > DJ.";

/// The voice Haze downloads when Piper has none set: clear, US English, about 60 MB.
const PIPER_VOICE: &str = "en_US-lessac-medium";
const PIPER_VOICE_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium";

pub async fn speak(config: &DjConfig, text: &str) -> Result<()> {
    match config.tts.engine {
        TtsEngine::System => system(config, text).await,
        TtsEngine::OpenAI => openai(config, text).await,
        TtsEngine::Piper => piper(config, text).await,
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
        if found.is_none() {
            log::warn!("no speech program: {}", last_err.map(|e| e.to_string()).unwrap_or_default());
        }
        found.ok_or_else(|| anyhow!(NO_SYSTEM_VOICE))?
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
    play(resp.bytes().await?.to_vec()).await
}

async fn piper(config: &DjConfig, text: &str) -> Result<()> {
    play(piper_audio(config, text).await?).await
}

/// Piper's speech as a WAV file.
async fn piper_audio(config: &DjConfig, text: &str) -> Result<Vec<u8>> {
    let program = find_piper(&config.tts.piper_path).await.ok_or_else(|| anyhow!(NO_PIPER))?;
    let voice = match non_empty(&config.tts.voice) {
        Some(v) => PathBuf::from(v),
        None => download_piper_voice().await?,
    };
    if !voice.is_file() {
        bail!("Piper voice file not found: {}", voice.display());
    }
    let wav = std::env::temp_dir().join(format!("haze-dj-{}.wav", std::process::id()));
    let mut child = Command::new(&program)
        .arg("-m")
        .arg(&voice)
        .arg("-f")
        .arg(&wav)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("running {}", program.display()))?;
    let mut stdin = child.stdin.take().context("Piper's input")?;
    // One line, since older Piper builds speak each input line separately.
    stdin.write_all(text.replace('\n', " ").as_bytes()).await?;
    drop(stdin);
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Piper failed: {}", err.lines().last().unwrap_or("no details"));
    }
    let audio = tokio::fs::read(&wav).await.context("reading Piper's audio")?;
    let _ = tokio::fs::remove_file(&wav).await;
    Ok(audio)
}

/// The Piper program, found once. Arch packages it as `piper-tts` because `piper` is also a mouse-settings app,
/// and apps opened from a menu or Finder may not see ~/.local/bin or Homebrew on PATH.
async fn find_piper(configured: &str) -> Option<PathBuf> {
    static FOUND: Mutex<Option<PathBuf>> = Mutex::new(None);
    if let Some(path) = non_empty(configured) {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = FOUND.lock().ok()?.clone() {
        return Some(path);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut candidates: Vec<PathBuf> = vec!["piper-tts".into(), "piper".into()];
    candidates.extend([
        home.join(".local/bin/piper"),
        "/opt/homebrew/bin/piper".into(),
        "/usr/local/bin/piper".into(),
        "/opt/piper-tts/piper".into(),
    ]);
    for program in candidates {
        let help = Command::new(&program).arg("--help").stdin(Stdio::null()).output();
        let Ok(Ok(out)) = tokio::time::timeout(Duration::from_secs(10), help).await else { continue };
        let help = [out.stdout, out.stderr].concat();
        if String::from_utf8_lossy(&help).contains("--model") {
            *FOUND.lock().ok()? = Some(program.clone());
            return Some(program);
        }
    }
    None
}

/// The default Piper voice, downloaded into Haze's `voices` folder the first time it's needed.
pub async fn download_piper_voice() -> Result<PathBuf> {
    let dir = config::data_dir("voices")?;
    let model = dir.join(format!("{PIPER_VOICE}.onnx"));
    let model_config = dir.join(format!("{PIPER_VOICE}.onnx.json"));
    if model.is_file() && model_config.is_file() {
        return Ok(model);
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).build()?;
    for file in [&model_config, &model] {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let resp = client.get(format!("{PIPER_VOICE_URL}/{name}")).send().await?.error_for_status()?;
        let bytes = resp.bytes().await.context("downloading the Piper voice")?;
        write_atomically(file, &bytes).await?;
    }
    Ok(model)
}

async fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let part = path.with_extension("part");
    tokio::fs::write(&part, bytes).await?;
    tokio::fs::rename(&part, path).await?;
    Ok(())
}

async fn play(audio: Vec<u8>) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let stream = rodio::OutputStreamBuilder::open_default_stream().context("opening audio output")?;
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.append(rodio::Decoder::new(Cursor::new(audio)).context("decoding speech audio")?);
        sink.sleep_until_end();
        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs Piper installed and HAZE_TEST_PIPER_VOICE set to a voice file.
    #[tokio::test]
    #[ignore]
    async fn piper_speaks() {
        let mut config = DjConfig::default();
        config.tts.engine = TtsEngine::Piper;
        config.tts.voice = std::env::var("HAZE_TEST_PIPER_VOICE").unwrap();
        let wav = piper_audio(&config, "Up next,\na little late-night jazz.").await.unwrap();
        assert!(rodio::Decoder::new(Cursor::new(wav)).is_ok());
    }
}
