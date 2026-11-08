//! Compare different models on the same image
//! Run: cargo test --test model_compare_test -- --nocapture

use std::path::Path;
use std::time::Instant;
use image::GenericImageView;

#[test]
fn compare_models() {
    let input = r"D:\2025Project\Lucid\src-tauri\target\debug\DSC04618.JPG";
    let tmp = std::env::temp_dir();
    
    let models = ["u2net", "isnet-general-use", "birefnet-general-lite"];
    
    println!("\n========== MODEL COMPARISON ==========");
    println!("Input: {input}\n");
    
    for model in &models {
        let out = tmp.join(format!("compare_{model}.png"));
        let t0 = Instant::now();
        let result = magidiva_lib::bg_remover::remove_background_with_model(
            Path::new(input), &out, model,
        );
        let elapsed = t0.elapsed();
        
        match result {
            Ok(()) => {
                let img = image::open(&out).unwrap();
                let (w, h) = img.dimensions();
                let rgba = img.to_rgba8();
                
                // Count "suspect" pixels: alpha=255 but RGB is dark (likely false foreground)
                let mut suspect = 0usize;
                for p in rgba.pixels() {
                    if p.0[3] > 200 && p.0[0] < 180 && p.0[1] < 180 && p.0[2] < 180 {
                        suspect += 1;
                    }
                }
                let total = w * h;
                let dark_ratio = suspect as f64 / total as f64 * 100.0;
                
                println!("{} -> {:.1}s | suspect dark opaque: {} ({:.2}%) | {}x{}",
                    model, elapsed.as_secs_f32(), suspect, dark_ratio, w, h);
            }
            Err(e) => println!("{} -> FAIL: {}", model, e),
        }
    }
    
    println!("\n========== DONE ==========");
}
