//! Local transcription with NVIDIA parakeet-unified-en-0.6b (English,
//! int8) through parakeet-rs. Picked after testing it against Parakeet
//! v2/v3, Cohere Transcribe and Qwen3-ASR on this Mac: most accurate,
//! fastest, and no dropped passages.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use parakeet_rs::{ParakeetUnified, TimestampMode, Transcriber};

use crate::progress::{MUTED, RESET};
use crate::text::Timed;

const MODEL: &str = "parakeet-unified-en-0.6b-int8";
/// The model's repo at a fixed commit, so the files can't change under us.
const BASE_URL: &str = "https://huggingface.co/bobNight/parakeet-unified-en-0.6b-onnx/resolve/09e9060322d99c5f070010724786e6ee090fd51d";
/// (file in the repo, name parakeet-rs looks for, SHA-256 at that commit).
/// The encoder's weights keep their original name because the encoder file
/// refers to them by it. Each download is checked before it's used.
const FILES: [(&str, &str, &str); 5] = [
    (
        "tokenizer.model",
        "tokenizer.model",
        "07d4e5a63840a53ab2d4d106d2874768143fb3fbdd47938b3910d2da05bfb0a9",
    ),
    (
        "vocab.txt",
        "vocab.txt",
        "ae61c9b743cb47c04ce6fb130444210646ed0f40c953c77ee1855185960af94f",
    ),
    (
        "decoder_joint.int8.onnx",
        "decoder_joint.onnx",
        "7f76ad5f35035f25630075699c6c942a2c0c05ff42cb398f966f3c256d148e1e",
    ),
    (
        "encoder.int8.onnx.data",
        "encoder.int8.onnx.data",
        "3d54dd04646c15677bd2844a84df3770b12cc1ce183481f7b6e0def31c92114a",
    ),
    (
        "encoder.int8.onnx",
        "encoder.onnx",
        "c81adfab77634e00c1668a221a14f244c5fb3409e7c14eeebaf6ac963425910f",
    ),
];
const SR: usize = 16_000;

/// ~/Library/Application Support/transclip
fn app_dir() -> Result<PathBuf> {
    let base =
        dirs::data_dir().ok_or_else(|| anyhow!("couldn't find the Application Support folder"))?;
    Ok(base.join("transclip"))
}

/// ~/Library/Application Support/transclip/models/parakeet-unified-en-0.6b-int8
pub fn model_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("models").join(MODEL))
}

/// Before the rename the model lived under "yt-transcribe". Move it across
/// rather than download 650 MB again.
fn move_old_model(dir: &Path) {
    let Some(base) = dirs::data_dir() else { return };
    let old_app = base.join("yt-transcribe");
    let old = old_app.join("models").join(MODEL);
    if dir.exists() || !old.is_dir() {
        return;
    }
    let moved = dir
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::rename(&old, dir));
    match moved {
        Ok(()) => {
            // Tidy up the old folders if they're now empty.
            let _ = std::fs::remove_dir(old_app.join("models"));
            let _ = std::fs::remove_dir(&old_app);
        }
        Err(e) => eprintln!(
            "{MUTED}•{RESET} Couldn't move the speech model from {} ({e}), so it will download again. You can delete that old folder afterwards.",
            old_app.display()
        ),
    }
}

/// Delete the downloaded model (and anything left from before the rename).
/// Returns how many bytes were freed.
pub fn delete_model() -> Result<u64> {
    let mut freed = 0;
    let mut dirs_to_go = vec![app_dir()?];
    if let Some(base) = dirs::data_dir() {
        dirs_to_go.push(base.join("yt-transcribe"));
    }
    for dir in dirs_to_go {
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            // Only the link is removed; the files it points to stay put.
            std::fs::remove_file(&dir)
                .with_context(|| format!("couldn't remove the link {}", dir.display()))?;
            eprintln!(
                "{MUTED}•{RESET} {} was a link to another folder. The link is gone, but the files it pointed to were left alone.",
                dir.display()
            );
            continue;
        }
        freed += folder_size(&dir);
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("couldn't delete {}", dir.display()))?;
    }
    Ok(freed)
}

fn folder_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => folder_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// Download any missing model files (one time, about 650 MB).
pub fn ensure_model() -> Result<PathBuf> {
    let dir = model_dir()?;
    move_old_model(&dir);
    let missing: Vec<(&str, &str, &str)> = FILES
        .iter()
        .copied()
        .filter(|(_, local, _)| !dir.join(local).is_file())
        .collect();
    if missing.is_empty() {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir)?;
    eprintln!(
        "{MUTED}•{RESET} First run: downloading NVIDIA's speech model (about 650 MB, once only)"
    );
    for (remote, local, sha256) in missing {
        // A staging file of its own for each attempt, so two runs downloading
        // at once can't overwrite each other's file before it's moved into
        // place. It's deleted on any error (TempPath) or interruption.
        let prefix = format!(".{local}.");
        let part = tempfile::Builder::new()
            .prefix(&prefix)
            .suffix(".part")
            .tempfile_in(&dir)?
            .into_temp_path();
        crate::cleanup::register(&part);
        let got = download_checked(remote, &part, sha256);
        crate::cleanup::unregister(&part);
        got?;
        part.persist(dir.join(local))?;
    }
    Ok(dir)
}

fn download_checked(remote: &str, part: &Path, sha256: &str) -> Result<()> {
    let status = Command::new("curl")
        .args(["-L", "--fail", "--progress-bar", "-o"])
        .arg(part)
        .arg(format!("{BASE_URL}/{remote}"))
        .status()
        .context("couldn't run curl")?;
    if !status.success() {
        bail!("Couldn't download the model file {remote}");
    }
    if file_sha256(part)? != sha256 {
        bail!(
            "The model file {remote} didn't match its expected fingerprint, so it wasn't used. Try again later."
        );
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Convert any audio file to the 16 kHz mono 16-bit WAV the model needs.
pub fn to_wav16k(input: &Path, output: &Path) -> Result<()> {
    let out = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(output)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!("ffmpeg isn't on your PATH")
            } else {
                e.into()
            }
        })?;
    if !out.status.success() {
        bail!(
            "ffmpeg couldn't convert the audio: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Read a 16 kHz mono 16-bit WAV as samples between -1 and 1.
pub fn read_wav(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).context("couldn't open the converted audio")?;
    reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0))
        .collect::<Result<_, _>>()
        .context("couldn't read the converted audio")
}

pub fn load(dir: &Path) -> Result<ParakeetUnified> {
    ParakeetUnified::from_pretrained(dir, None)
        .map_err(|e| anyhow!("couldn't load the speech model: {e}"))
}

/// Transcribe in roughly 30-second pieces, cutting at the quietest moment
/// near each mark so words aren't split. Testing showed 30-second pieces
/// drop less speech than longer ones. `progress` gets 0.0..=1.0.
pub fn run(
    model: &mut ParakeetUnified,
    samples: &[f32],
    mut progress: impl FnMut(f64),
) -> Result<Vec<Timed>> {
    const TARGET: usize = 30 * SR;
    const WINDOW: usize = 5 * SR / 2;
    const FRAME: usize = 480; // 30 ms

    let mut out = Vec::new();
    let mut pos = 0;
    progress(0.0);
    while pos < samples.len() {
        let rest = &samples[pos..];
        let len = if rest.len() <= TARGET + WINDOW {
            rest.len()
        } else {
            quiet_split(rest, TARGET, WINDOW, FRAME)
        };
        if len >= SR / 4 {
            let r = model
                .transcribe_samples(
                    rest[..len].to_vec(),
                    SR as u32,
                    1,
                    Some(TimestampMode::Sentences),
                )
                .map_err(|e| anyhow!("transcription failed: {e}"))?;
            let offset = pos as f32 / SR as f32;
            let before = out.len();
            for s in r.tokens {
                let text = s.text.trim();
                if !text.is_empty() {
                    out.push(Timed {
                        start: s.start + offset,
                        end: s.end + offset,
                        text: text.to_string(),
                    });
                }
            }
            let t = r.text.trim();
            if out.len() == before && !t.is_empty() {
                out.push(Timed {
                    start: offset,
                    end: offset + len as f32 / SR as f32,
                    text: t.to_string(),
                });
            }
        }
        pos += len;
        progress(pos as f64 / samples.len() as f64);
    }
    Ok(out)
}

/// Index of the quietest 30 ms frame within `window` of `target`.
fn quiet_split(s: &[f32], target: usize, window: usize, frame: usize) -> usize {
    let start = (target - window) / frame * frame;
    let stop = (target + window).min(s.len());
    let (mut best, mut min) = (target, f32::MAX);
    let mut o = start;
    while o + frame <= stop {
        let f = &s[o..o + frame];
        let rms = (f.iter().map(|x| x * x).sum::<f32>() / frame as f32).sqrt();
        if rms < min {
            min = rms;
            best = o + frame / 2;
        }
        o += frame;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_files() {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), b"abc").unwrap();
        assert_eq!(
            file_sha256(f.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn splits_at_the_quiet_spot() {
        let mut s = vec![0.5f32; 100 * SR];
        let quiet = 31 * SR;
        for x in &mut s[quiet..quiet + 480] {
            *x = 0.0;
        }
        let cut = quiet_split(&s, 30 * SR, 5 * SR / 2, 480);
        assert!(cut >= quiet && cut <= quiet + 480, "cut at {cut}");
    }
}
