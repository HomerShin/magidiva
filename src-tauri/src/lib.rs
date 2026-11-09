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
) -> Result<Vec<ProcessResult>, String> {
    check_pro_license()?;
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
    .map(|v| Ok(v))
    .unwrap_or_else(|e| Err(format!("Batch cancelled: {e}")))
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
fn walk_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    check_pro_license()?;
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
    Ok(out)
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

// ──────────────────────────────────────────────────
// License activation — shared by Community & Pro builds.
// Pro build gates batch/gallery on license.valid.
// Community build always returns valid=true (no gating needed).
// ──────────────────────────────────────────────────

/// License server base URL — deployed as Cloudflare Worker.
const LICENSE_SERVER: &str = "https://magidiva-license.805998855.workers.dev";

/// Client-side HMAC public key (used for quick local signature check — reserved).
/// Server private key generates the signature — never shipped to clients.
#[allow(dead_code)]
const CLIENT_HMAC_KEY: &[u8] = b"magiDiva-CLIENT-PUBLIC-VERIFY-KEY-v1";

/// License persisted to disk.
#[derive(Serialize, Deserialize, Clone, Default)]
struct LicenseFile {
    email: String,
    key: String,
    plan: String,
    verified_at: Option<i64>,
    /// Signed payload from server — client can HMAC-verify locally
    local_token: Option<String>,
    /// Seconds of grace — if last online verification older than this, lock Pro
    grace_seconds: i64,
}

fn license_path() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("MagiDiva");
    Some(dir.join("license.json"))
}

/// Quick in-process gate — called by batch_process/walk_paths to enforce
/// activation before allowing Pro-only operations.
#[cfg(feature = "pro")]
fn check_pro_license() -> Result<(), String> {
    let Some(path) = license_path() else {
        return Err("Pro activation required — no license file found".into());
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Err("Pro activation required — cannot read license".into());
    };
    let Ok(lic) = serde_json::from_str::<LicenseFile>(&raw) else {
        return Err("Pro activation required — invalid license file".into());
    };
    // Local token present + within grace → allow
    if lic.local_token.is_some() {
        let now = chrono::Utc::now().timestamp();
        if let Some(v) = lic.verified_at {
            if now - v <= lic.grace_seconds {
                return Ok(());
            }
        }
    }
    Err("Pro activation required — please re-activate".into())
}

#[allow(dead_code)]
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("valid key length");
    mac.update(msg);
    mac.finalize().into_bytes().into()
}

/// Verify a local_token signature client-side (fast path — no network).
#[allow(dead_code)]
fn verify_local_token(email: &str, plan: &str, expires_ts: i64, token: &str) -> bool {
    // token format: hex( HMAC(email|plan|expires) )
    let expected = hmac_sha256(
        CLIENT_HMAC_KEY,
        format!("{email}|{plan}|{expires_ts}").as_bytes(),
    );
    let expected_hex = hex::encode(expected);
    expected_hex == token
}

#[tauri::command]
fn get_license() -> LicenseStatus {
    // Community build — always fully licensed, no activation needed
    if !cfg!(feature = "pro") {
        return LicenseStatus {
            valid: true, plan: "community".into(),
            email: None, expires_at: None,
            message: "Community edition".into(),
        };
    }

    let Some(path) = license_path() else {
        return LicenseStatus { valid: false, plan: "pro".into(), email: None, expires_at: None,
            message: "No license found".into() };
    };

    let Ok(raw) = std::fs::read_to_string(&path) else {
        return LicenseStatus { valid: false, plan: "pro".into(), email: None, expires_at: None,
            message: "Cannot read license file".into() };
    };
    let Ok(lic) = serde_json::from_str::<LicenseFile>(&raw) else {
        return LicenseStatus { valid: false, plan: "pro".into(), email: None, expires_at: None,
            message: "Invalid license file".into() };
    };

    // Check local signature first (fast)
    if lic.local_token.is_some() {
        // If grace period exceeded, mark invalid (frontend will prompt re-activate)
        let now = chrono::Utc::now().timestamp();
        let grace_expired = lic.verified_at.map_or(true, |v| now - v > lic.grace_seconds);

        return LicenseStatus {
            valid: !grace_expired,
            plan: lic.plan.clone(),
            email: Some(lic.email.clone()),
            expires_at: lic.verified_at.map(|v| v + lic.grace_seconds),
            message: if grace_expired { "Offline grace expired — re-activate".into() } else { "OK".into() },
        };
    }

    LicenseStatus { valid: false, plan: "pro".into(), email: None, expires_at: None,
        message: "No local token".into() }
}

#[derive(Serialize, Clone)]
struct LicenseStatus {
    valid: bool,
    plan: String,
    email: Option<String>,
    expires_at: Option<i64>,
    message: String,
}

#[tauri::command]
fn activate_license(email: String, key: String) -> Result<LicenseStatus, String> {
    if !cfg!(feature = "pro") {
        return Ok(LicenseStatus { valid: true, plan: "community".into(), email: Some(email), expires_at: None,
            message: "Community — no activation needed".into() });
    }

    // Call server verify endpoint
    let url = format!("{}/api/license/verify", LICENSE_SERVER);
    let body = serde_json::json!({ "email": email, "key": key });
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("Network error: {e}"))?;

    let resp_json: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap_or_default())
        .map_err(|e| format!("Bad response: {e}"))?;

    let valid = resp_json["valid"].as_bool().unwrap_or(false);
    let plan = resp_json["plan"].as_str().unwrap_or("pro").to_string();

    if !valid {
        return Err(resp_json["message"].as_str().unwrap_or("License key invalid").to_string());
    }

    let now = chrono::Utc::now().timestamp();
    let token = resp_json["token"].as_str().map(|s| s.to_string());

    let lic = LicenseFile {
        email: email.clone(),
        key: key.clone(),
        plan: plan.clone(),
        verified_at: Some(now),
        local_token: token,
        grace_seconds: 7 * 24 * 3600, // 7 days grace
    };

    // Persist
    if let Some(path) = license_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&lic).unwrap_or_default());
    }

    Ok(LicenseStatus {
        valid: true, plan, email: Some(email),
        expires_at: Some(now + lic.grace_seconds),
        message: "License activated ✓".into(),
    })
}

#[tauri::command]
fn clear_license() -> Result<(), String> {
    if let Some(path) = license_path() {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
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
                get_license,
                activate_license,
                clear_license,
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
                get_license,
                activate_license,
                clear_license,
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}
