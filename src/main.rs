mod recorder;

use recorder::{quick_record, high_quality_record, ScreenRecorder, RecorderConfig};
use std::time::Duration;
use anyhow::Result;

fn main() -> Result<()> {
    println!("🎬 Screen Recorder with ffmpeg-next");
    println!("=====================================");

    // Просто вызываем быструю запись
    demo_quick_record()
}

fn demo_quick_record() -> Result<()> {
    println!("🚀 Starting quick recording (30 FPS, medium quality)...");

    // Создаем output директорию если нет
    std::fs::create_dir_all("output")?;

    // Записываем 10 секунд (Ctrl+C для ранней остановки)
    quick_record("output/demo_quick.mp4", 30)?;

    Ok(())
}

#[allow(dead_code)]
fn demo_high_quality() -> Result<()> {
    println!("🎥 Starting high quality recording (60 FPS, CRF 18)...");

    std::fs::create_dir_all("output")?;

    // Запись с высоким качеством
    high_quality_record("output/demo_high_quality.mp4", 60)?;

    Ok(())
}

#[allow(dead_code)]
fn demo_custom_config() -> Result<()> {
    println!("⚙️  Starting recording with custom configuration...");

    let config = RecorderConfig::new(
        "output/demo_custom.mp4".to_string(),
        24, // 24 FPS
        Some("default".to_string()) // Аудио источник
    ).with_quality(20); // Качество между средним и высоким

    let mut recorder = ScreenRecorder::new(config)?;
    recorder.start()?;

    Ok(())
}

#[allow(dead_code)]
fn demo_multiple_qualities() -> Result<()> {
    println!("📊 Recording with different quality levels...");

    std::fs::create_dir_all("output")?;

    let qualities = vec![
        ("output/low_quality.mp4", 30, 28),
        ("output/medium_quality.mp4", 30, 23),
        ("output/high_quality.mp4", 30, 18),
    ];

    for (output, fps, quality) in qualities {
        println!("🎥 Recording: {} (CRF: {})", output, quality);

        let config = RecorderConfig::new(output.to_string(), fps, None)
            .with_quality(quality);

        let mut recorder = ScreenRecorder::new(config)?;
        recorder.start()?;

        println!("⏱️  Waiting 2 seconds before next recording...\n");
        std::thread::sleep(Duration::from_secs(2));
    }

    Ok(())
}

#[allow(dead_code)]
fn demo_interactive() -> Result<()> {
    println!("🎮 Interactive Screen Recorder");
    println!("==============================");

    // Проверяем есть ли inquire для интерактивного режима
    #[cfg(feature = "inquire")]
    {
        use inquire::{Text, Select, Confirm};

        let output = Text::new("Output file path:")
            .with_default("output/interactive.mp4")
            .prompt()?;

        let fps_choice = Select::new("Target FPS:", &["24", "30", "60", "120"])
            .prompt()?;
        let fps = fps_choice.parse::<u32>()?;

        let quality = Select::new("Video quality:", &[
            "Low (CRF 28) - smaller file",
            "Medium (CRF 23) - balanced",
            "High (CRF 18) - best quality",
            "Ultra (CRF 15) - maximum quality"
        ]).prompt()?;

        let crf = quality.split('(').nth(1).unwrap_or("23")
            .split(')').next().unwrap_or("23")
            .trim()
            .parse::<u32>()
            .unwrap_or(23);

        println!("\n🎥 Starting recording with:");
        println!("   Output: {}", output);
        println!("   FPS: {}", fps);
        println!("   Quality: CRF {}", crf);

        let config = RecorderConfig::new(output, fps, None).with_quality(crf);
        let mut recorder = ScreenRecorder::new(config)?;
        recorder.start()?;
    }

    #[cfg(not(feature = "inquire"))]
    {
        println!("❌ Interactive mode requires 'inquire' feature");
        println!("Add to Cargo.toml: inquire = \"0.6\"");
        return demo_quick_record();
    }

    Ok(())
}

#[allow(dead_code)]
fn demo_error_handling() -> Result<()> {
    println!("🛡️  Testing error handling...");

    // Попытка записи в несуществующую директорию
    let bad_config = RecorderConfig::new(
        "/nonexistent/path/output.mp4".to_string(),
        30,
        None
    );

    match ScreenRecorder::new(bad_config) {
        Ok(_) => println!("❌ Should have failed!"),
        Err(e) => {
            println!("✅ Correctly handled error: {}", e);
            println!("🔄 Trying with valid path...");

            // Fallback на рабочий путь
            std::fs::create_dir_all("output")?;
            let good_config = RecorderConfig::new(
                "output/fallback.mp4".to_string(),
                30,
                None
            );

            let mut recorder = ScreenRecorder::new(good_config)?;
            recorder.start()?;
        }
    }

    Ok(())
}