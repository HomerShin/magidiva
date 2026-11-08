use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context};
use image::{RgbaImage, Rgba, GrayImage, Luma, DynamicImage, GenericImageView};

/// Platform presets: (name, w, h)
pub const SIZE_PRESETS: &[(&str, u32, u32)] = &[
    ("Original", 0, 0),
    ("Shopify 2048x2048", 2048, 2048),
    ("Shopify 1024x1024", 1024, 1024),
    ("Etsy 2000x2000", 2000, 2000),
    ("Amazon 1000x1000", 1000, 1000),
    ("Amazon Main 2000x2000", 2000, 2000),
    ("Square 800x800", 800, 800),
    ("Square 500x500", 500, 500),
];

/// Preset shadow styles. Numbers are multipliers applied to canvas dimensions.
#[derive(Debug, Clone, Copy)]
pub enum ShadowStyle {
    /// 几乎看不出 — 适合小件/精致商品
    Soft,
    /// 平衡的柔光 — 默认
    Medium,
    /// 厚重投影 — 大件商品
    Strong,
    /// 只有硬一点的偏移
    Offset,
}

impl Default for ShadowStyle {
    fn default() -> Self { Self::Medium }
}

impl ShadowStyle {
    /// Returns (sigma_ratio, dy_ratio, strength)
    /// sigma = min(w,h) * sigma_ratio    (clamped later)
    /// dy    = h * dy_ratio
    pub fn params(self) -> (f32, f32, f32) {
        match self {
            Self::Soft   => (0.010, 0.006, 0.20),
            Self::Medium => (0.018, 0.010, 0.30),
            Self::Strong => (0.030, 0.018, 0.42),
            Self::Offset => (0.006, 0.020, 0.28),
        }
    }
}

pub fn parse_shadow_style(s: &str) -> ShadowStyle {
    match s {
        "soft"   => ShadowStyle::Soft,
        "strong" => ShadowStyle::Strong,
        "offset" => ShadowStyle::Offset,
        _        => ShadowStyle::Medium,
    }
}

/// Composite an RGBA cutout over a solid background.
/// When `shadow` is on, a drop shadow derived from the alpha channel
/// (blur + downward offset) is painted onto the background canvas before the product.
pub fn compose_on_bg(rgba: &RgbaImage, shadow: bool, style: ShadowStyle, bg: (u8, u8, u8)) -> RgbaImage {
    let (w, h) = rgba.dimensions();
    let mut canvas: RgbaImage = RgbaImage::from_pixel(w, h, Rgba([bg.0, bg.1, bg.2, 255]));

    if shadow && w > 4 && h > 4 {
        let mut alpha: GrayImage = GrayImage::new(w, h);
        for (x, y, p) in rgba.enumerate_pixels() {
            alpha.put_pixel(x, y, Luma([p.0[3]]));
        }
        let (sigma_ratio, dy_ratio, strength) = style.params();
        let sigma = (w.min(h) as f32 * sigma_ratio).clamp(1.5, 80.0);
        let blurred = image::imageops::blur(&alpha, sigma);
        let dy = (h as f32 * dy_ratio).max(1.0) as i64;

        for (x, y, px) in canvas.enumerate_pixels_mut() {
            let sy = y as i64 - dy;
            if sy < 0 { continue; }
            let a = blurred.get_pixel(x, sy as u32)[0] as f32 / 255.0 * strength;
            if a <= 0.0 { continue; }
            // Shadow blends the bg color toward black — preserves tint on non-white backgrounds
            let bg_v = [bg.0 as f32, bg.1 as f32, bg.2 as f32];
            let darken = |v: f32| (v * (1.0 - a * 0.85)).round() as u8;
            *px = Rgba([darken(bg_v[0]), darken(bg_v[1]), darken(bg_v[2]), 255]);
        }
    }

    // Standard source-over composite of the product onto the (possibly shadowed) canvas
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p.0[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let bg = canvas.get_pixel(x, y).0;
        let blend = |fg: u8, bg: u8| (fg as f32 * a + bg as f32 * (1.0 - a)).round() as u8;
        *canvas.get_pixel_mut(x, y) = Rgba([
            blend(p.0[0], bg[0]),
            blend(p.0[1], bg[1]),
            blend(p.0[2], bg[2]),
            255,
        ]);
    }

    canvas
}

/// Encode an RGBA buffer as png / jpeg / webp.
/// `quality` (1-100) only applies to JPEG; webp here is lossless.
pub fn save_encoded(img: &RgbaImage, output: &Path, format: &str, quality: u8) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).ok();
    }

    match format {
        "jpg" | "jpeg" => {
            let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            let file = fs::File::create(output)
                .with_context(|| format!("create {}", output.display()))?;
            let mut writer = std::io::BufWriter::new(file);
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut writer,
                quality.clamp(1, 100),
            );
            rgb.write_with_encoder(enc)
                .with_context(|| format!("save {}", output.display()))?;
        }
        "webp" => {
            let file = fs::File::create(output)
                .with_context(|| format!("create {}", output.display()))?;
            let mut writer = std::io::BufWriter::new(file);
            let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut writer);
            img.write_with_encoder(enc)
                .with_context(|| format!("save {}", output.display()))?;
        }
        _ => {
            img.save(output)
                .with_context(|| format!("save {}", output.display()))?;
        }
    }
    Ok(())
}

/// 清理 RGBA resize 产生的边缘污染：
/// Triangle/Lanczos 等滤镜在 alpha 边缘会把前景 RGB 插值到透明区域
/// white_bg 时半透明深色像素 + 白色 = 灰色残留
/// 解法：alpha <= 128 的像素直接置 (0,0,0,0)，alpha > 128 的 alpha 拉满 255
fn clean_alpha_edges(rgba: &RgbaImage) -> RgbaImage {
    let mut out = rgba.clone();
    for p in out.pixels_mut() {
        if p.0[3] <= 128 {
            *p = Rgba([0, 0, 0, 0]);
        } else {
            p.0[3] = 255;
        }
    }
    out
}

/// Auto-center: 保持原图尺寸不裁透明边，等比缩放后贴 canvas 中心
/// 关键：不裁 bbox，这样产品在 canvas 里的相对位置和原图一致
pub fn auto_center(input: &Path, output: &Path, target_w: u32, target_h: u32) -> Result<()> {
    let img = image::open(input).with_context(|| format!("open {}", input.display()))?;
    let rgba = img.to_rgba8();
    let (orig_w, orig_h) = rgba.dimensions();

    if target_w == 0 || target_h == 0 {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).ok();
        }
        rgba.save(output).with_context(|| format!("save {}", output.display()))?;
        return Ok(());
    }

    // 等比缩放（只缩小不放大），保持完整透明边 → 产品相对位置不变
    let pad_ratio = 0.95;
    let max_scale = (target_w as f32 / orig_w as f32).min(target_h as f32 / orig_h as f32) * pad_ratio;
    let scale = max_scale.min(1.0);
    let new_w = ((orig_w as f32 * scale) as u32).max(1);
    let new_h = ((orig_h as f32 * scale) as u32).max(1);

    let resized = image::imageops::resize(&rgba, new_w, new_h, image::imageops::FilterType::Triangle);
    let resized = clean_alpha_edges(&resized);

    // Center on canvas
    let mut canvas: RgbaImage = RgbaImage::new(target_w, target_h);
    let offset_x = (target_w - new_w) / 2;
    let offset_y = (target_h - new_h) / 2;
    for (x, y, pixel) in resized.enumerate_pixels() {
        let cx = x + offset_x;
        let cy = y + offset_y;
        if cx < target_w && cy < target_h {
            *canvas.get_pixel_mut(cx, cy) = *pixel;
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).ok();
    }
    canvas.save(output).with_context(|| format!("save {}", output.display()))?;
    Ok(())
}

/// Resize image to target dimensions (letterbox, no bbox crop)
pub fn resize_to(input: &Path, output: &Path, target_w: u32, target_h: u32) -> Result<()> {
    let img = image::open(input).with_context(|| format!("open {}", input.display()))?;
    let (orig_w, orig_h) = img.dimensions();

    if target_w == 0 || target_h == 0 {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).ok();
        }
        img.save(output).with_context(|| format!("save {}", output.display()))?;
        return Ok(());
    }

    let scale = (target_w as f32 / orig_w as f32).min(target_h as f32 / orig_h as f32);
    let new_w = ((orig_w as f32 * scale) as u32).max(1);
    let new_h = ((orig_h as f32 * scale) as u32).max(1);

    let rgba = img.to_rgba8();
    let resized = image::imageops::resize(&rgba, new_w, new_h, image::imageops::FilterType::Triangle);
    let resized = clean_alpha_edges(&resized);

    let mut canvas: RgbaImage = RgbaImage::new(target_w, target_h);
    let offset_x = (target_w - new_w) / 2;
    let offset_y = (target_h - new_h) / 2;
    for (x, y, pixel) in resized.enumerate_pixels() {
        let cx = x + offset_x;
        let cy = y + offset_y;
        if cx < target_w && cy < target_h {
            *canvas.get_pixel_mut(cx, cy) = *pixel;
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).ok();
    }
    canvas.save(output).with_context(|| format!("save {}", output.display()))?;
    Ok(())
}

/// Chain: remove_bg → (optional center) → (optional white_bg) → (optional resize)
pub fn process_pipeline(
    input: &Path,
    output: &Path,
    opts: &ProcessOptions,
    on_stage: &dyn Fn(crate::bg_remover::Stage),
) -> Result<()> {
    use crate::bg_remover;

    // Step 1: remove background
    let need_intermediate = opts.center || opts.white_bg || opts.target_w > 0;
    let cutout_path = if need_intermediate {
        let p = std::env::temp_dir().join(format!("magidiva_pipe_{}.png", std::process::id()));
        bg_remover::remove_background_with_model(input, &p, &opts.model_name, on_stage)?;
        p
    } else {
        bg_remover::remove_background_with_model(input, output, &opts.model_name, on_stage)?;
        return Ok(());
    };

    // Step 2 (optional): auto-center / resize
    let mut temp_paths: Vec<PathBuf> = Vec::new();
    let after_transform: PathBuf = {
        if opts.center && opts.target_w > 0 && opts.target_h > 0 {
            let p = std::env::temp_dir().join(format!("magidiva_ctr_{}.png", std::process::id()));
            auto_center(&cutout_path, &p, opts.target_w, opts.target_h)?;
            temp_paths.push(p.clone());
            p
        } else if opts.target_w > 0 && opts.target_h > 0 {
            let p = std::env::temp_dir().join(format!("magidiva_rsz_{}.png", std::process::id()));
            resize_to(&cutout_path, &p, opts.target_w, opts.target_h)?;
            temp_paths.push(p.clone());
            p
        } else {
            cutout_path.clone()
        }
    };

    // Step 3: final render (white bg / shadow / watermark) and encode in the requested format
    on_stage(crate::bg_remover::Stage::Saving);
    let fmt = normalize_format(&opts.format);
    // JPEG 没有透明通道，必须压到白底上
    let flatten = opts.white_bg || opts.shadow || fmt == "jpg";
    let needs_render = flatten || opts.watermark_path.is_some() || fmt != "png";

    if !needs_render {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::copy(&after_transform, output)
            .with_context(|| format!("save {}", output.display()))?;
    } else {
        let img = image::open(&after_transform)
            .with_context(|| format!("open {}", after_transform.display()))?
            .to_rgba8();
        let style = parse_shadow_style(&opts.shadow_style);
        let mut rendered = if flatten { compose_on_bg(&img, opts.shadow, style, opts.bg_color) } else { img };

        // Step 4 (optional): watermark overlay, composited before encoding
        if let Some(ref wm_path) = opts.watermark_path {
            rendered = apply_watermark(
                &rendered,
                wm_path,
                opts.watermark_opacity,
                opts.watermark_position.as_deref(),
            )?;
        }
        save_encoded(&rendered, output, fmt, opts.quality)?;
    }

    // Cleanup all temp files
    let _ = fs::remove_file(&cutout_path);
    for p in &temp_paths {
        let _ = fs::remove_file(p);
    }

    Ok(())
}

/// Overlay a watermark image onto the target and return the composited buffer.
/// `opacity` is 0.0–1.0, `position` is one of:
///   tl / t / tr / l / c / r / bl / b / br  (9-grid anchors)
pub fn apply_watermark(
    base: &RgbaImage,
    watermark: &Path,
    opacity: f32,
    position: Option<&str>,
) -> Result<RgbaImage> {
    let mut base = base.clone();
    let wm = image::open(watermark).with_context(|| format!("open watermark {}", watermark.display()))?.to_rgba8();
    let (bw, bh) = base.dimensions();
    let (ww, wh) = wm.dimensions();

    // Resize watermark to 15% of base width, maintaining aspect
    let ratio = 0.15f32;
    let new_ww = ((bw as f32 * ratio) as u32).max(1);
    let new_wh = ((wh as f32 * (new_ww as f32 / ww as f32)) as u32).max(1);
    let wm_resized = image::imageops::resize(&wm, new_ww, new_wh, image::imageops::FilterType::Lanczos3);

    // Anchor position mapping (with 24px margin)
    let margin = 24u32;
    let (ox, oy) = match position.unwrap_or("br") {
        "tl" => (margin, margin),
        "t"  => ((bw - new_ww) / 2, margin),
        "tr" => (bw - new_ww - margin, margin),
        "l"  => (margin, (bh - new_wh) / 2),
        "c"  => ((bw - new_ww) / 2, (bh - new_wh) / 2),
        "r"  => (bw - new_ww - margin, (bh - new_wh) / 2),
        "bl" => (margin, bh - new_wh - margin),
        "b"  => ((bw - new_ww) / 2, bh - new_wh - margin),
        "br" => (bw - new_ww - margin, bh - new_wh - margin),
        _    => (bw - new_ww - margin, bh - new_wh - margin),
    };

    // Alpha-blend watermark onto base
    for (x, y, wp) in wm_resized.enumerate_pixels() {
        let bx = ox + x;
        let by = oy + y;
        if bx >= bw || by >= bh { continue; }

        let base_px = base.get_pixel_mut(bx, by);
        let wm_alpha = (wp.0[3] as f32 / 255.0) * opacity;
        let ba = base_px.0[3] as f32 / 255.0;

        // Blend watermark over base (standard over operator)
        let out_alpha = wm_alpha + ba * (1.0 - wm_alpha);
        if out_alpha > 0.0 {
            base_px.0[0] = ((wp.0[0] as f32 * wm_alpha + base_px.0[0] as f32 * ba * (1.0 - wm_alpha)) / out_alpha) as u8;
            base_px.0[1] = ((wp.0[1] as f32 * wm_alpha + base_px.0[1] as f32 * ba * (1.0 - wm_alpha)) / out_alpha) as u8;
            base_px.0[2] = ((wp.0[2] as f32 * wm_alpha + base_px.0[2] as f32 * ba * (1.0 - wm_alpha)) / out_alpha) as u8;
            base_px.0[3] = (out_alpha * 255.0) as u8;
        }
    }

    Ok(base)
}

/// Normalize a user-supplied format string to the supported output set.
pub fn normalize_format(format: &str) -> &str {
    match format {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub white_bg: bool,
    pub shadow: bool,
    /// "soft" / "medium" / "strong" / "offset"
    pub shadow_style: String,
    /// Background RGB when white_bg is on, e.g. (255,255,255)
    pub bg_color: (u8, u8, u8),
    pub center: bool,
    pub target_w: u32,
    pub target_h: u32,
    pub watermark_path: Option<PathBuf>,
    pub watermark_opacity: f32,
    pub watermark_position: Option<String>,
    pub model_name: String,
    /// png / jpg / webp
    pub format: String,
    /// JPEG quality 1-100
    pub quality: u8,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            white_bg: false,
            shadow: false,
            shadow_style: "medium".to_string(),
            bg_color: (255, 255, 255),
            center: false,
            target_w: 0,
            target_h: 0,
            watermark_path: None,
            watermark_opacity: 0.7,
            watermark_position: None,
            model_name: "birefnet-general-lite".to_string(),
            format: "png".to_string(),
            quality: 92,
        }
    }
}
