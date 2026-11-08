use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context};

/// Complete per-model inference config (from rembg Python source)
pub struct ModelInfo {
    pub url: &'static str,
    pub input_w: u32,
    pub input_h: u32,
    pub mean: [f32; 3],
    pub std: [f32; 3],
    pub uses_sigmoid: bool,
    pub description: &'static str,
}

pub const MODELS: &[(&str, ModelInfo)] = &[
    // --- Fast, already downloaded ---
    ("u2net", ModelInfo {
        url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2net.onnx",
        input_w: 320, input_h: 320,
        mean: [0.485, 0.456, 0.406], std: [0.229, 0.224, 0.225],
        uses_sigmoid: true,
        description: "Fast baseline, 320×320, good for simple product shots (~176MB)",
    }),
    // --- Better quality ---
    ("isnet-general-use", ModelInfo {
        url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx",
        input_w: 1024, input_h: 1024,
        mean: [0.5, 0.5, 0.5], std: [1.0, 1.0, 1.0],
        uses_sigmoid: true,
        description: "General purpose, clean edges, 1024×1024 (~170MB)",
    }),
    // --- Best quality ---
    ("birefnet-general-lite", ModelInfo {
        url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx",
        input_w: 1024, input_h: 1024,
        mean: [0.485, 0.456, 0.406], std: [0.229, 0.224, 0.225],
        uses_sigmoid: true,
        description: "BiRefNet lite, best quality/size tradeoff (~214MB)",
    }),
];

pub fn get_model_info(name: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|(n, _)| *n == name).map(|(_, info)| info)
}

pub fn list_models() -> Vec<(String, u32, String)> {
    MODELS.iter().map(|(name, info)| {
        (name.to_string(), info.input_w, info.description.to_string())
    }).collect()
}

/// Download model to ~/.magidiva/models if not present
pub fn ensure_model(
    model_name: &str,
    on_download: &dyn Fn(f32),
) -> Result<(PathBuf, &'static ModelInfo)> {
    let info = get_model_info(model_name)
        .with_context(|| format!("unknown model: {model_name}"))?;

    let models_dir = models_dir()?;
    fs::create_dir_all(&models_dir).context("create models dir")?;

    let ext = info.url.rsplit('.').next().unwrap_or("onnx");
    let model_path = models_dir.join(format!("{model_name}.{ext}"));
    if !model_path.exists() {
        eprintln!("[model] downloading {model_name} ({}) ...", info.url);
        let resp = ureq::get(info.url)
            .call()
            .with_context(|| format!("download {model_name}"))?;

        let total = resp.header("content-length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let mut reader = resp.into_reader();
        let mut file = fs::File::create(&model_path)?;
        // 分块拷贝以便上报下载进度（首次运行要下 170-214MB，没有反馈会以为卡死）
        let mut buf = vec![0u8; 64 * 1024];
        let mut read_bytes: u64 = 0;
        loop {
            let n = std::io::Read::read(&mut reader, &mut buf)?;
            if n == 0 { break; }
            std::io::Write::write_all(&mut file, &buf[..n])?;
            read_bytes += n as u64;
            if total > 0 {
                on_download((read_bytes as f64 / total as f64 * 100.0) as f32);
            }
        }

        eprintln!(
            "[model] saved {} ({:.1} MB)",
            model_path.display(),
            total as f64 / 1_048_576.0
        );
    }

    Ok((model_path, info))
}

fn models_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .context("resolve home dir")?;
    Ok(Path::new(&home).join(".magidiva").join("models"))
}
