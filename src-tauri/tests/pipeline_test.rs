// Integration test: full pipeline (cutout → white bg → resize)
#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Instant;
    use image::GenericImageView;

    #[test]
    fn full_pipeline_test() {
        let input = r"D:\2025Project\authentix\Authentix\models\prnu_ref\refs\C01_Huawei_P30lite\flat_01_1_1.jpg";
        if !Path::new(input).exists() {
            eprintln!("[test] SKIP — test image not found");
            return;
        }

        let output = std::env::temp_dir().join("magidiva_pipeline_1024.png");
        if output.exists() { std::fs::remove_file(&output).ok(); }

        let opts = magidiva_lib::processor::ProcessOptions {
            white_bg: true,
            target_w: 1024,
            target_h: 1024,
        };

        println!("[test] pipeline: cutout → white bg → 1024x1024");
        println!("[test] input:  {}", input);
        println!("[test] output: {}", output.display());

        let t0 = Instant::now();
        let result = magidiva_lib::processor::process_pipeline(
            Path::new(input),
            &output,
            &opts,
        );
        let elapsed = t0.elapsed();

        match result {
            Ok(()) => {
                let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
                let img = image::open(&output).unwrap();
                let (w, h) = img.dimensions();
                println!(
                    "[test] PASS in {:.1}s ({} bytes, {}x{})",
                    elapsed.as_secs_f32(),
                    size,
                    w,
                    h
                );
                assert_eq!(w, 1024, "width should be 1024");
                assert_eq!(h, 1024, "height should be 1024");
            }
            Err(e) => {
                panic!("[test] FAIL: {e}");
            }
        }
    }

    #[test]
    fn cutout_only_test() {
        let input = r"D:\2025Project\authentix\Authentix\models\prnu_ref\refs\C01_Huawei_P30lite\flat_01_1_1.jpg";
        if !Path::new(input).exists() {
            eprintln!("[test] SKIP — test image not found");
            return;
        }

        let output = std::env::temp_dir().join("magidiva_cutout_only.png");
        if output.exists() { std::fs::remove_file(&output).ok(); }

        let opts = magidiva_lib::processor::ProcessOptions::default();

        let t0 = Instant::now();
        let result = magidiva_lib::processor::process_pipeline(
            Path::new(input),
            &output,
            &opts,
        );
        let elapsed = t0.elapsed();

        match result {
            Ok(()) => {
                let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
                println!("[test] PASS in {:.1}s ({} bytes, cutout only)", elapsed.as_secs_f32(), size);
                assert!(output.exists());
            }
            Err(e) => panic!("[test] FAIL: {e}"),
        }
    }
}
