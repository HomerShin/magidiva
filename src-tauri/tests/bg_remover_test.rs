// Quick smoke test: run bg_remover on a real image
// Run with: cargo test --manifest-path src-tauri/Cargo.toml bg_remover_test -- --nocapture

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Instant;

    #[test]
    fn bg_remover_test() {
        // Find a test image
        let candidates = [
            r"D:\2025Project\authentix\Authentix\models\prnu_ref\refs\C01_Huawei_P30lite\flat_01_1_1.jpg",
        ];
        let input = candidates.iter().find(|p| Path::new(p).exists())
            .expect("no test image found");

        let output = std::env::temp_dir().join("magidiva_test_cutout.png");
        if output.exists() { std::fs::remove_file(&output).ok(); }

        println!("[test] input:  {}", input);
        println!("[test] output: {}", output.display());

        let t0 = Instant::now();
        let result = magidiva_lib::bg_remover::remove_background(
            Path::new(input),
            &output,
        );
        let elapsed = t0.elapsed();

        match result {
            Ok(()) => {
                let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
                println!("[test] PASS in {:.1}s (output {} bytes)", elapsed.as_secs_f32(), size);
                assert!(output.exists(), "output file should exist");
                assert!(size > 1024, "output too small ({size} bytes)");
            }
            Err(e) => {
                eprintln!("[test] FAIL: {e}");
                panic!("test failed: {e}");
            }
        }
    }
}
