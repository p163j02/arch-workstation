//! GTK3 lives in a separate, inert process: GTK3 and GTK4 cannot share a process.
//! Only visual settings cross the boundary; no exec/on-click commands are sent.
use crate::model::Document;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Frame {
    pub png: Vec<u8>,
    pub width: i32,
    pub height: i32,
}

pub struct Reply {
    pub id: u64,
    pub result: Result<Frame, String>,
}

struct Job {
    id: u64,
    snapshot: Value,
    directory: PathBuf,
}

pub struct Renderer {
    wake: SyncSender<()>,
    pending: Arc<Mutex<Option<Job>>>,
    replies: Receiver<Reply>,
    latest: Arc<AtomicU64>,
}

impl Renderer {
    pub fn new() -> Self {
        // Exactly one pending snapshot, not an unbounded queue of CSS copies.
        let pending: Arc<Mutex<Option<Job>>> = Arc::new(Mutex::new(None));
        let latest = Arc::new(AtomicU64::new(0));
        let (wake, jobs) = mpsc::sync_channel(1);
        let (results, replies) = mpsc::channel();
        let worker_pending = pending.clone();
        let worker_latest = latest.clone();
        std::thread::spawn(move || {
            while jobs.recv().is_ok() {
                let Some(mut job) = worker_pending.lock().unwrap().take() else {
                    continue;
                };
                let started = Instant::now();
                // Debounce, with an upper bound so continuous edits cannot starve work.
                while jobs.recv_timeout(Duration::from_millis(120)).is_ok() {
                    if let Some(newer) = worker_pending.lock().unwrap().take() {
                        job = newer;
                    }
                    if started.elapsed() >= Duration::from_millis(250) {
                        break;
                    }
                }
                let result = render(&job, &worker_latest);
                if worker_latest.load(Ordering::Relaxed) == job.id
                    && results.send(Reply { id: job.id, result }).is_err()
                {
                    break;
                }
            }
        });
        Self {
            wake,
            pending,
            replies,
            latest,
        }
    }

    pub fn request(
        &self,
        document: &Document,
        directory: PathBuf,
        width: i32,
        height: i32,
        scenario: &str,
    ) {
        let id = self.latest.fetch_add(1, Ordering::Relaxed) + 1;
        let snapshot = snapshot(document, width, height, scenario);
        *self.pending.lock().unwrap() = Some(Job {
            id,
            snapshot,
            directory,
        });
        let _ = self.wake.try_send(());
    }

    pub fn invalidate(&self) {
        self.latest.fetch_add(1, Ordering::Relaxed);
        self.pending.lock().unwrap().take();
    }

    pub fn poll(&self) -> Option<Reply> {
        let mut latest = None;
        while let Ok(reply) = self.replies.try_recv() {
            if reply.id == self.latest.load(Ordering::Relaxed) {
                latest = Some(reply);
            }
        }
        latest
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.invalidate();
    }
}

fn snapshot(document: &Document, width: i32, height: i32, scenario: &str) -> Value {
    let root = document.value().unwrap_or(json!({}));
    let mut config = serde_json::Map::new();
    for (key, value) in root.as_object().into_iter().flatten() {
        if [
            "position",
            "fixed-center",
            "spacing",
            "modules-left",
            "modules-center",
            "modules-right",
        ]
        .contains(&key.as_str())
        {
            config.insert(key.clone(), value.clone());
        } else if let Some(spec) = value.as_object() {
            let visual = spec
                .iter()
                .filter(|(k, _)| {
                    k.as_str() == "format"
                        || k.starts_with("format-")
                        || [
                            "persistent-workspaces",
                            "states",
                            "spacing",
                            "icon-size",
                            "modules",
                        ]
                        .contains(&k.as_str())
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>();
            config.insert(key.clone(), Value::Object(visual));
        }
    }
    let vertical = matches!(root["position"].as_str(), Some("left" | "right"));
    let (w, h) = if vertical {
        (
            root["width"].as_i64().unwrap_or(48).clamp(24, 512),
            i64::from(height).clamp(320, 4096),
        )
    } else {
        let margin = root["margin-left"].as_i64().unwrap_or(0).clamp(0, 512)
            + root["margin-right"].as_i64().unwrap_or(0).clamp(0, 512);
        (
            root["width"]
                .as_i64()
                .filter(|w| *w > 0)
                .unwrap_or(i64::from(width) - margin)
                .clamp(320, 8192),
            root["height"].as_i64().unwrap_or(40).clamp(1, 512),
        )
    };
    json!({"config": config, "css": document.css, "width": w, "height": h, "scenario": scenario})
}

fn render(job: &Job, latest: &AtomicU64) -> Result<Frame, String> {
    if latest.load(Ordering::Relaxed) != job.id {
        return Err("Vista reemplazada".into());
    }
    let input = serde_json::to_vec(&job.snapshot).map_err(|e| e.to_string())?;
    if input.len() > 4 * 1024 * 1024 {
        return Err("El CSS o la configuración son demasiado grandes para la vista GTK3.".into());
    }
    let mut child = Command::new("python3")
        .args(["-c", include_str!("preview_renderer.py")])
        .env(
            "WAYBAR_EDITOR_PREVIEW_PARENT",
            std::process::id().to_string(),
        )
        .current_dir(&job.directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Se necesita Python3/PyGObject con GTK3 para esta vista: {e}"))?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(3 * 1024 * 1024).read_to_end(&mut bytes);
        bytes
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(16 * 1024).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    let status = loop {
        if latest.load(Ordering::Relaxed) != job.id {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let _ = writer.join();
    let png = reader.join().unwrap_or_default();
    let error = errors.join().unwrap_or_default();
    if status.is_some_and(|s| s.success())
        && png.starts_with(b"\x89PNG\r\n\x1a\n")
        && png.len() >= 24
    {
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        if width <= 8192 && height <= 4096 && width > 0 && height > 0 {
            return Ok(Frame {
                png,
                width: width as i32,
                height: height as i32,
            });
        }
    }
    if status.is_none() {
        Err("GTK3 tardó demasiado. Se conserva la vista esquemática.".into())
    } else {
        Err(format!(
            "No se pudo dibujar el CSS con GTK3: {}",
            error.chars().take(1600).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_excludes_all_execution_and_click_settings() {
        let document = Document {
            config: json!({"modules-right": ["custom/test"], "custom/test": {
                "format": "{}", "exec": "touch /tmp/should-not-run", "on-click": "danger",
                "on-scroll-up": "danger", "restart-interval": 1, "format-icons": ["a","b"]
            }, "height": 40, "margin-left": 8, "margin-right": 8})
            .to_string(),
            css: "* { color: #fff; }".into(),
        };
        let result = snapshot(&document, 1920, 1080, "normal");
        assert_eq!(result["width"], 1904);
        assert!(result["config"]["custom/test"].get("exec").is_none());
        assert!(result["config"]["custom/test"].get("on-click").is_none());
        assert!(
            result["config"]["custom/test"]
                .get("on-scroll-up")
                .is_none()
        );
        assert_eq!(result["config"]["custom/test"]["format"], "{}");
        assert_eq!(result["css"], document.css);
    }

    #[test]
    fn snapshot_bounds_geometry_and_supports_vertical_layout() {
        let document = Document {
            config: r#"{"position":"left","width":100000}"#.into(),
            css: String::new(),
        };
        let result = snapshot(&document, 1920, 1080, "critical");
        assert_eq!(result["width"], 512);
        assert_eq!(result["height"], 1080);
        assert_eq!(result["scenario"], "critical");
    }
}
