pub mod model;
pub mod bg_remover;
pub mod processor;

use std::path::PathBuf;
#[cfg(feature = "pro")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "pro")]
use std::sync::Arc;
use serde::{Serialize, Deserialize};
#[cfg(feature = "pro")]
use tauri::{AppHandle, Emitter};
#[cfg(not(feature = "pro"))]
use tauri::AppHandle;

/// Global cancellation flag — Pro only. Community build has no batch so no need.
#[cfg(feature = "pro")]
fn cancel_flag() -> Arc<AtomicBool> {
    use std::sync::OnceLock;
    static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    FLAG.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

#[derive(Serialize, Clone)]
struct ProcessResult {
    input: String,
    output: String,
    success: bool,
    error: Option<String>,
}

#[cfg(feature = "pro")]
#[derive(Serialize, Clone)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum ProgressPhase {
    Starting { model: String },
    Downloading { model: String, percent: u8 },
    Inferring,
    Saving,
    Done { success: bool, error: Option<String> },
}

#[cfg(feature = "pro")]
#[derive(Serialize, Clone)]
struct BatchProgress {
    current: usize,
    total: usize,
    input: String,
    phase: ProgressPhase,
}

#[derive(Deserialize, Default, Clone)]
struct ProcessOpts {
    #[serde(default)] white_bg: bool,
    #[serde(default)] shadow: bool,
    #[serde(default = "default_shadow_style")] shadow_style: String,
    #[serde(default = "default_bg_r")] bg_r: u8,
    #[serde(default = "default_bg_g")] bg_g: u8,
    #[serde(default = "default_bg_b")] bg_b: u8,
    #[serde(default)] center: bool,
    #[serde(default)] target_w: u32,
    #[serde(default)] target_h: u32,
    #[serde(default)] watermark_path: Option<String>,
    #[serde(default = "default_opacity")] watermark_opacity: f32,
    #[serde(default)] watermark_position: Option<String>,
    #[serde(default = "default_model")] model_name: String,
    #[serde(default = "default_format")] format: String,
    #[serde(default = "default_quality")] quality: u8,
}

fn default_opacity() -> f32 { 0.7 }
fn default_model() -> String { "birefnet-general-lite".into() }
fn default_format() -> String { "png".into() }
fn default_quality() -> u8 { 92 }
fn default_shadow_style() -> String { "medium".into() }
fn default_bg_r() -> u8 { 255 }
fn default_bg_g() -> u8 { 255 }
fn default_bg_b() -> u8 { 255 }

fn to_proc_opts(o: &ProcessOpts) -> processor::ProcessOptions {
    processor::ProcessOptions {
        white_bg: o.white_bg,
        shadow: o.shadow,
        shadow_style: o.shadow_style.clone(),
        bg_color: (o.bg_r, o.bg_g, o.bg_b),
        center: o.center,
        target_w: o.target_w,
        target_h: o.target_h,
        watermark_path: o.watermark_path.as_ref().map(PathBuf::from),
        watermark_opacity: o.watermark_opacity,
        watermark_position: o.watermark_position.clone(),
        model_name: o.model_name.clone(),
        format: processor::normalize_format(&o.format).to_string(),
        quality: o.quality,
    }
}

#[tauri::command]
fn process_image(
    input: String,
    output: String,
    opts: Option<ProcessOpts>,
) -> ProcessResult {
    let in_path = PathBuf::from(&input);
    let out_path = PathBuf::from(&output);
    let proc_opts = to_proc_opts(&opts.unwrap_or_default());

    match processor::process_pipeline(&in_path, &out_path, &proc_opts, &|_| {}) {
        Ok(()) => ProcessResult {
            input, output: out_path.to_string_lossy().into_owned(),
            success: true, error: None,
        },
        Err(e) => ProcessResult {
            input, output, success: false,
            error: Some(format!("{e}")),
        },
    }
}

#[cfg(feature = "pro")]
#[tauri::command]
async fn batch_process(
    app: AppHandle,
    inputs: Vec<String>,
    output_dir: String,
    opts: Option<ProcessOpts>,
) -> Vec<ProcessResult> {
    let flag = cancel_flag();
    flag.store(false, Ordering::Relaxed);

    let cancel_clone = flag.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let out_dir = PathBuf::from(&output_dir);
        let proc_opts = to_proc_opts(&opts.unwrap_or_default());
        let total = inputs.len();
        let mut results = Vec::new();
        for (idx, input) in inputs.into_iter().enumerate() {
            if cancel_clone.load(Ordering::Relaxed) {
                let _ = app.emit("batch-cancelled", ());
                break;
            }
            let in_path = PathBuf::from(&input);
            let current = idx + 1;

            let stem = in_path.file_stem().unwrap_or_default().to_string_lossy();
            let mut parts: Vec<String> = Vec::new();
            if proc_opts.white_bg {
                let (r, g, b) = proc_opts.bg_color;
                if (r, g, b) != (255, 255, 255) {
                    parts.push(format!("bg_{r:02x}{g:02x}{b:02x}"));
                }
                if proc_opts.shadow {
                    parts.push(format!("shadow_{}", proc_opts.shadow_style));
                }
            }
            if proc_opts.center {
                parts.push("centered".into());
            }
            if proc_opts.watermark_path.is_some() {
                parts.push("wm".into());
            }
            let suffix = if parts.is_empty() {
                "_cutout".into()
            } else {
                format!("_{}", parts.join("_"))
            };
            let ext = processor::normalize_format(&proc_opts.format);
            let output_path = out_dir.join(format!("{stem}{suffix}.{ext}"));

            let _ = app.emit("batch-progress", BatchProgress {
                current, total, input: input.clone(),
                phase: ProgressPhase::Starting { model: proc_opts.model_name.clone() },
            });

            let on_stage = |st: bg_remover::Stage| {
                let phase = match st {
                    bg_remover::Stage::Downloading(percent) => ProgressPhase::Downloading {
                        model: proc_opts.model_name.clone(),
                        percent: percent.clamp(0.0, 100.0) as u8,
                    },
                    bg_remover::Stage::Inferring => ProgressPhase::Inferring,
                    bg_remover::Stage::Saving => ProgressPhase::Saving,
                };
                let _ = app.emit("batch-progress", BatchProgress {
                    current, total, input: input.clone(), phase,
                });
            };
            let result = match processor::process_pipeline(&in_path, &output_path, &proc_opts, &on_stage) {
                Ok(()) => ProcessResult {
                    input: input.clone(),
                    output: output_path.to_string_lossy().into_owned(),
                    success: true, error: None,
                },
                Err(e) => ProcessResult {
                    input: input.clone(),
                    output: output_path.to_string_lossy().into_owned(),
                    success: false,
                    error: Some(format!("{e}")),
                },
            };

            let _ = app.emit("batch-progress", BatchProgress {
                current, total, input: input.clone(),
                phase: ProgressPhase::Done {
                    success: result.success,
                    error: result.error.clone(),
                },
            });

            results.push(result);
        }
        results
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn list_size_presets() -> Vec<(String, u32, u32)> {
    processor::SIZE_PRESETS
        .iter()
        .map(|(name, w, h)| (name.to_string(), *w, *h))
        .collect()
}

#[tauri::command]
fn list_models() -> Vec<(String, u32, String)> {
    model::list_models()
}

#[tauri::command]
fn list_wm_positions() -> Vec<(String, String)> {
    vec![
        ("Bottom Right".into(), "br".into()),
        ("Bottom Left".into(), "bl".into()),
        ("Bottom Center".into(), "b".into()),
        ("Top Right".into(), "tr".into()),
        ("Top Left".into(), "tl".into()),
        ("Top Center".into(), "t".into()),
        ("Middle Left".into(), "l".into()),
        ("Middle Right".into(), "r".into()),
        ("Dead Center".into(), "c".into()),
    ]
}

#[tauri::command]
async fn open_file(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(&path, None::<&str>).map_err(|e| format!("{e}"))
}

#[tauri::command]
async fn reveal_in_folder(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let p = PathBuf::from(&path);
    let folder = p.parent()
        .ok_or_else(|| "invalid path".to_string())?;
    app.opener().open_path(folder.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| format!("{e}"))
}

#[cfg(feature = "pro")]
#[tauri::command]
fn walk_paths(paths: Vec<String>) -> Vec<String> {
    use std::collections::HashSet;
    let exts: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    let is_img = |ext: Option<&str>| -> bool {
        ext.map(|e| exts.iter().any(|ok| ok.eq_ignore_ascii_case(e))).unwrap_or(false)
    };
    for p in paths {
        let path = PathBuf::from(&p);
        if path.is_file() {
            if is_img(path.extension().and_then(|e| e.to_str())) && seen.insert(p.clone()) {
                out.push(p);
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let full = entry.path().to_string_lossy().into_owned();
                    if is_img(entry.path().extension().and_then(|e| e.to_str())) && seen.insert(full.clone()) {
                        out.push(full);
                    }
                }
            }
        }
    }
    out
}

/// Pro only — signal an in-flight batch_process to stop after the current image.
#[cfg(feature = "pro")]
#[tauri::command]
fn cancel_batch() {
    cancel_flag().store(true, Ordering::Relaxed);
}

/// Frontend asks "am I running Community or Pro?" — drives UI gating.
#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        is_pro: cfg!(feature = "pro"),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[derive(Serialize, Clone)]
struct AppInfo {
    is_pro: bool,
    version: String,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! Welcome to MagiDiva ✨")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(not(feature = "pro"))]
    {
        tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init())
            .invoke_handler(tauri::generate_handler![
                greet,
                process_image,
                list_size_presets,
                list_wm_positions,
                open_file,
                reveal_in_folder,
                list_models,
                app_info,
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }

    #[cfg(feature = "pro")]
    {
        tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init())
            .invoke_handler(tauri::generate_handler![
                greet,
                process_image,
                batch_process,
                cancel_batch,
                walk_paths,
                list_size_presets,
                list_wm_positions,
                open_file,
                reveal_in_folder,
                list_models,
                app_info,
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}
