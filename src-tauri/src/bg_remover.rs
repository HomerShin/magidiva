use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::fs;
use anyhow::{Result, Context, anyhow};
use image::{GenericImageView, RgbaImage, ImageBuffer, Rgba};
use ort::session::Session;
use ort::value::Tensor;
use ort::ep::DirectML;

use crate::model::{ensure_model, ModelInfo};

/// 处理阶段，用于向上层上报进度
#[derive(Clone, Copy)]
pub enum Stage {
    /// 首次运行下载模型，0-100
    Downloading(f32),
    /// 加载模型 + 推理
    Inferring,
    /// 写出结果
    Saving,
}

struct SessionCache {
    session: Session,
    info: &'static ModelInfo,
}

fn sessions() -> &'static Mutex<HashMap<String, SessionCache>> {
    static MAP: OnceLock<Mutex<HashMap<String, SessionCache>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ort_env() -> &'static ort::environment::Environment {
    static ENV: OnceLock<Mutex<Option<ort::environment::Environment>>> = OnceLock::new();
    let slot = ENV.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().expect("ort env lock poisoned");
    if guard.is_none() {
        let env = ort::init().build().expect("ort::init failed");
        *guard = Some(env);
    }
    // Leak the &'static reference — safe because it's initialized once and never dropped
    let env_ref: &ort::environment::Environment = guard.as_ref().unwrap();
    unsafe { &*(env_ref as *const ort::environment::Environment) }
}

fn load_session(model_name: &str, on_download: &dyn Fn(f32)) -> Result<SessionCache> {
    let (model_path, info) = ensure_model(model_name, on_download)?;
    let env = ort_env();

    let sess = match Session::builder(env)
        .map_err(|e| anyhow!("session builder: {e}"))?
        .with_execution_providers([DirectML::default().build()])
        .map_err(|e| anyhow!("directml build: {e}"))?
        .commit_from_file(&model_path)
    {
        Ok(s) => { eprintln!("[ort:{model_name}] DirectML OK"); s }
        Err(e) => {
            eprintln!("[ort:{model_name}] DirectML failed ({e}), falling back to CPU");
            Session::builder(&env)
                .map_err(|e| anyhow!("session builder: {e}"))?
                .commit_from_file(&model_path)
                .map_err(|e| anyhow!("cpu session: {e}"))?
        }
    };

    Ok(SessionCache { session: sess, info })
}

/// Remove background with specified model
pub fn remove_background_with_model(
    input_path: &Path,
    output_path: &Path,
    model_name: &str,
    on_stage: &dyn Fn(Stage),
) -> Result<()> {
    // --- 1. Load image ---
    let img = image::open(input_path)
        .with_context(|| format!("open {}", input_path.display()))?;
    let (orig_w, orig_h) = img.dimensions();

    // --- 2. Get / load session (with correct per-model preprocessing) ---
    on_stage(Stage::Inferring);
    let mut map = sessions().lock().map_err(|e| anyhow!("lock sessions: {e}"))?;
    let cache = if let Some(c) = map.get_mut(model_name) {
        c
    } else {
        let loaded = load_session(model_name, &|pct| on_stage(Stage::Downloading(pct)))?;
        map.insert(model_name.to_string(), loaded);
        map.get_mut(model_name).unwrap()
    };

    let info = cache.info;
    let session = &mut cache.session;

    let input_w_us = info.input_w as usize;
    let input_h_us = info.input_h as usize;

    // --- 3. Preprocess with per-model mean/std ---
    // 必须用 resize_exact（精确拉伸到模型输入尺寸），不能用 resize。
    // resize 保持宽高比，2816x2112 会缩成 1024x768，导致 tensor 底部 25% 仍是 0.0（中灰），
    // 模型看到"上半截人 + 下半截灰块"会误判，在头顶上方吐出前景块 → 白底后变成深色残留。
    // resize_exact 与 rembg 的做法一致。
    let resized = img.resize_exact(
        info.input_w,
        info.input_h,
        image::imageops::FilterType::Triangle,
    );
    let rgb = resized.to_rgb8();

    let mut tensor_data = vec![0.0f32; 3 * input_w_us * input_h_us];
    for (x, y, pixel) in rgb.enumerate_pixels() {
        let idx = y as usize * input_w_us + x as usize;
        tensor_data[idx] = ((pixel.0[0] as f32 / 255.0) - info.mean[0]) / info.std[0];
        tensor_data[input_w_us * input_h_us + idx] =
            ((pixel.0[1] as f32 / 255.0) - info.mean[1]) / info.std[1];
        tensor_data[2 * input_w_us * input_h_us + idx] =
            ((pixel.0[2] as f32 / 255.0) - info.mean[2]) / info.std[2];
    }

    // --- 4. Run inference ---
    let input_tensor = Tensor::from_array((
        [1usize, 3, input_h_us, input_w_us],
        tensor_data.into_boxed_slice(),
    ))
    .map_err(|e| anyhow!("create tensor: {e}"))?;

    let outputs = session
        .run(ort::inputs![input_tensor])
        .map_err(|e| anyhow!("run model: {e}"))?;

    // --- 5. Extract mask ---
    let output_value = &outputs[0];
    let (_shape, mask_flat) = output_value
        .try_extract_tensor::<f32>()
        .map_err(|e| anyhow!("extract mask: {e}"))?;

    // --- 6. Post-process ---
    let pixel_count = input_w_us * input_h_us;
    let mut processed = Vec::with_capacity(pixel_count);
    for &v in mask_flat.iter() {
        let p = if info.uses_sigmoid { 1.0 / (1.0 + (-v).exp()) } else { v };
        processed.push(p);
    }

    // If NOT using sigmoid, min-max normalize to [0, 1]
    if !info.uses_sigmoid {
        let mut min_v = f32::INFINITY;
        let mut max_v = f32::NEG_INFINITY;
        for &v in &processed { min_v = min_v.min(v); max_v = max_v.max(v); }
        let range = (max_v - min_v).max(1e-8);
        for v in processed.iter_mut() { *v = (*v - min_v) / range; }
    }

    // Apply threshold + feather (smooth transition)
    // mask < 0.3 → fully transparent, mask > 0.6 → fully opaque, linear feather between
    // lo=0.3 更激进地砍掉背景残留（之前 0.4 太保守，头顶/边缘残留没压干净）
    let lo = 0.3f32;
    let hi = 0.6f32;
    let span = hi - lo;
    for v in processed.iter_mut() {
        if *v <= lo { *v = 0.0; }
        else if *v >= hi { *v = 1.0; }
        else { *v = (*v - lo) / span; } // linear feather 0→1
    }

    let mask_u8: Vec<u8> = processed.iter().map(|&v| (v * 255.0) as u8).collect();
    let mask_img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(info.input_w, info.input_h, mask_u8)
            .context("build mask image")?;
    // Triangle 双线性插值会在 resize 后产生灰色中间值，导致边缘残留
    // 用 Lanczos3 缩放，再对 resize 后的 mask 重新跑 threshold 清除插值残留
    let mask_resized = image::imageops::resize(
        &mask_img, orig_w, orig_h, image::imageops::FilterType::Lanczos3,
    );

    // Second pass threshold：resize 插值在边缘造出半透明渐变，这里硬切全干掉
    // 完全二值化：alpha <= 128 全部置 0，> 128 全部置 255
    let mut mask_final = {
        let mut buf = mask_resized.clone();
        for p in buf.pixels_mut() {
            let a = p.0[0];
            let na = if a <= 128 { 0u8 } else { 255u8 };
            *p = image::Luma([na]);
        }
        buf
    };

    // Morphological erosion (kernel=1)：干掉边缘被 AI 误判为前景的小残留
    // 任何前景像素如果 5×5 邻域里有 >= 5 个背景像素，说明是孤岛残留 → 置 0
    // 这能处理"放大后残留变显眼"的根因——把残留从源头清掉
    {
        let (mw, mh) = (mask_final.width(), mask_final.height());
        let orig = mask_final.clone();
        for y in 1..mh.saturating_sub(1) {
            for x in 1..mw.saturating_sub(1) {
                if orig.get_pixel(x, y).0[0] == 255 {
                    // Count background in 3x3 neighborhood
                    let mut bg_count = 0;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if orig.get_pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32).0[0] == 0 {
                                bg_count += 1;
                            }
                        }
                    }
                    if bg_count >= 5 {
                        *mask_final.get_pixel_mut(x, y) = image::Luma([0]);
                    }
                }
            }
        }
    }

    // --- 7. Composite ---
    let original_rgba = img.to_rgba8();
    let mut out_rgba: RgbaImage = RgbaImage::new(orig_w, orig_h);
    for (x, y, out_pixel) in out_rgba.enumerate_pixels_mut() {
        let orig = original_rgba.get_pixel(x, y);
        let alpha = mask_final.get_pixel(x, y).0[0];
        *out_pixel = Rgba([orig.0[0], orig.0[1], orig.0[2], alpha]);
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    on_stage(Stage::Saving);
    out_rgba.save(output_path)
        .with_context(|| format!("save {}", output_path.display()))?;

    Ok(())
}
