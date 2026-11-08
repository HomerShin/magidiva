//! Diagnostic test: process user's exact image, dump every pipeline stage
//! Run: cargo test --test diagnose_test -- --nocapture

use std::path::Path;
use std::time::Instant;
use image::RgbaImage;

fn dump_alpha_stats(label: &str, img: &RgbaImage) {
    let mut zero = 0usize;
    let mut near_zero = 0usize; // alpha 1-30
    let mut mid = 0usize;       // alpha 31-224
    let mut near_full = 0usize; // alpha 225-254
    let mut full = 0usize;      // alpha 255
    let mut gray_rgb = 0usize;  // semi-transparent pixels with non-white RGB (potential residue)
    
    let total = img.width() as usize * img.height() as usize;
    for p in img.pixels() {
        let a = p.0[3];
        match a {
            0 => zero += 1,
            1..=30 => near_zero += 1,
            31..=224 => {
                mid += 1;
                // If semi-transparent AND RGB is dark (not white background), this is residue
                if p.0[0] < 200 || p.0[1] < 200 || p.0[2] < 200 {
                    gray_rgb += 1;
                }
            }
            225..=254 => near_full += 1,
            255 => full += 1,
        }
    }
    
    println!("  [{label}] total={total}");
    println!("    alpha=0 (fully transparent):    {zero:>8} ({:.1}%)", zero as f64 / total as f64 * 100.0);
    println!("    alpha 1-30 (near-zero):          {near_zero:>8} ({:.1}%)", near_zero as f64 / total as f64 * 100.0);
    println!("    alpha 31-224 (semi-transparent): {mid:>8} ({:.1}%) ← residue zone", mid as f64 / total as f64 * 100.0);
    println!("      └─ with dark RGB (actual residue): {gray_rgb:>6}", );
    println!("    alpha 225-254 (near-full):       {near_full:>8} ({:.1}%)", near_full as f64 / total as f64 * 100.0);
    println!("    alpha=255 (fully opaque):        {full:>8} ({:.1}%)", full as f64 / total as f64 * 100.0);
}

#[test]
fn diagnose_user_image() {
    // User's exact input
    let input = r"D:\2025Project\Lucid\src-tauri\target\debug\DSC04618.JPG";
    
    // First find the actual file
    let real_input = if Path::new(input).exists() {
        input.to_string()
    } else {
        // Try to find it
        let candidates = [
            r"E:\MaskWizTest\images\101MSDCF\DSC04618.JPG",
            r"E:\MaskWizTest\images\101MSDCF\DSC04618.jpg",
        ];
        candidates.iter().find(|p| Path::new(p).exists())
            .expect("Cannot find user's test image! Put DSC04618.JPG somewhere accessible")
            .to_string()
    };
    
    let tmp = std::env::temp_dir();
    let stage1_cutout = tmp.join("diag_stage1_cutout.png");
    let stage2_center = tmp.join("diag_stage2_centered.png");
    let stage3_whitebg = tmp.join("diag_stage3_whitebg.png");
    let final_out = tmp.join("diag_final.png");
    
    // Clean old
    for f in [&stage1_cutout, &stage2_center, &stage3_whitebg, &final_out] {
        let _ = std::fs::remove_file(f);
    }
    
    println!("\n========== MAGIDIVA DIAGNOSTIC ==========");
    println!("Input: {}", real_input);
    
    // Stage 1: bg removal only
    println!("\n--- STAGE 1: remove_background (no post-processing) ---");
    let t0 = Instant::now();
    magidiva_lib::bg_remover::remove_background_with_model(
        Path::new(&real_input), &stage1_cutout, "birefnet-general-lite",
    ).expect("stage1 failed");
    println!("  Done in {:.1}s", t0.elapsed().as_secs_f32());
    
    let img1 = image::open(&stage1_cutout).unwrap().to_rgba8();
    let (w1, h1) = img1.dimensions();
    println!("  Size: {w1}x{h1}");
    dump_alpha_stats("cutout", &img1);
    
    // Stage 2: auto-center + white bg + 2048x2048 (mimic full pipeline)
    println!("\n--- STAGE 2: WITH CENTER (auto-center + white bg + 2048x2048) ---");
    let t1 = Instant::now();
    
    let opts_center = magidiva_lib::processor::ProcessOptions {
        white_bg: true,
        center: true,
        target_w: 2048,
        target_h: 2048,
        ..Default::default()
    };
    let out_center = tmp.join("diag_with_center.png");
    magidiva_lib::processor::process_pipeline(
        Path::new(&real_input), &out_center, &opts_center,
    ).expect("pipeline with center failed");
    println!("  Done in {:.1}s", t1.elapsed().as_secs_f32());
    
    let img_c = image::open(&out_center).unwrap().to_rgba8();
    let (wc, hc) = img_c.dimensions();
    println!("  Size: {wc}x{hc}");
    dump_alpha_stats("WITH center (white bg)", &img_c);

    // Stage 3: WITHOUT center (just cutout → white bg → resize)
    println!("\n--- STAGE 3: WITHOUT CENTER (cutout + white bg + 2048x2048) ---");
    let t2 = Instant::now();
    
    let opts_nocenter = magidiva_lib::processor::ProcessOptions {
        white_bg: true,
        center: false,
        target_w: 2048,
        target_h: 2048,
        ..Default::default()
    };
    let out_nocenter = tmp.join("diag_no_center.png");
    magidiva_lib::processor::process_pipeline(
        Path::new(&real_input), &out_nocenter, &opts_nocenter,
    ).expect("pipeline no-center failed");
    println!("  Done in {:.1}s", t2.elapsed().as_secs_f32());
    
    let img_nc = image::open(&out_nocenter).unwrap().to_rgba8();
    let (wn, hn) = img_nc.dimensions();
    println!("  Size: {wn}x{hn}");
    dump_alpha_stats("WITHOUT center (white bg)", &img_nc);

    // Compare actual pixel content for residue
    println!("\n--- STAGE 4: Compare dark pixel counts ---");
    // Count dark pixels (RGB all < 200) in each output
    fn count_dark(img: &RgbaImage) -> usize {
        let mut n = 0usize;
        for p in img.pixels() {
            if p.0[0] < 200 && p.0[1] < 200 && p.0[2] < 200 && p.0[3] > 200 {
                n += 1;
            }
        }
        n
    }
    let dark_center = count_dark(&img_c);
    let dark_nocenter = count_dark(&img_nc);
    let ratio_c = dark_center as f64 / (wc*hc) as f64 * 100.0;
    let ratio_nc = dark_nocenter as f64 / (wn*hn) as f64 * 100.0;
    println!("  WITH center dark opaque pixels: {dark_center:>7} ({ratio_c:.3}%)");
    println!("  WITHOUT center dark opaque pixels: {dark_nocenter:>7} ({ratio_nc:.3}%)");
    
    println!("\n  Files in temp dir:");
    for f in &[&out_center, &out_nocenter] {
        let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        println!("    {} ({} KB)", f.file_name().unwrap().to_string_lossy(), size / 1024);
    }
}
