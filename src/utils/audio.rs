//! Shrinks sermon audio on the server with ffmpeg (speech-quality Opus).

use std::{path::Path, time::Duration};

use crate::errors::{AppError, AppResult};

/// Speech does not need stereo or a high bitrate: mono 32 kbps Opus sounds clear and is ~14 MB per hour.
pub async fn compress_to_opus(ffmpeg: &str, input: &Path, output: &Path) -> AppResult<()> {
    let mut cmd = tokio::process::Command::new(ffmpeg);
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args([
            "-vn", "-map_metadata", "-1", "-ac", "1",
            "-c:a", "libopus", "-b:a", "32k", "-vbr", "on",
            "-compression_level", "10", "-application", "voip", "-f", "ogg",
        ])
        .arg(output)
        .kill_on_drop(true);

    let run = tokio::time::timeout(Duration::from_secs(15 * 60), cmd.output()).await;
    match run {
        Err(_) => Err(AppError::Validation("Compressing the audio took too long. Try a shorter file.".into())),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotConfigured("Audio compression (ffmpeg)")),
        Ok(Err(e)) => Err(AppError::Internal(format!("could not run ffmpeg: {e}"))),
        Ok(Ok(out)) if out.status.success() => Ok(()),
        Ok(Ok(out)) => {
            tracing::warn!(stderr = %String::from_utf8_lossy(&out.stderr), "ffmpeg failed");
            Err(AppError::Validation("That file could not be read as audio. Please use an MP3, M4A, WAV or similar file.".into()))
        }
    }
}
