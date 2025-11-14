use scrap::{Display, Capturer};
use std::{io::{ErrorKind::WouldBlock, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};

// Говорим, что если все пойдет нормально, то мы вернем (), а
// если нет, то любую ошибку реализующую интерфейс Error
// Это нужно, чтобы удобно обрабатывать ошибки с помощью ?
pub fn record_screen() -> Result<(), Box<dyn std::error::Error>> {

    let display = Display::primary()?; // Синтаксический сахар
    let mut capturer = Capturer::new(display)?;

    let (width, height) = (capturer.width(), capturer.height());
    println!("Размеры экрана {}x{}", width, height);

    // Настраиваем ffmpeg
    // -f rawvideo: принимаем сырые кадры
    // -pixel_format rgba: 4 байта на пиксель
    // -video_size: размер кадра
    // -framerate 30: FPS
    // -i - : читаем видео через stdin
    // -c: v h264_nvec (или libx264): кодек
    // -preset fast: профиль кодировния
    // -y output.mp4: выходной файл

    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-y",
            "-init_hw_device", "vaapi=va:/dev/dri/renderD128",
            "-filter_hw_device", "va",
            "-f", "rawvideo",
            "-pixel_format", "bgr0",
            "-video_size", &format!("{width}x{height}"),
            "-framerate", "30",
            "-i", "-", // stdin
            // "-vf", "format=nv12,hwupload,scale_vaapi=w=1920:h=1080",
            "-vf", "format=nv12,hwupload=extra_hw_frames=16",
            "-c:v", "h264_vaapi", // libx264
            "-qp", "23", 
            "output/output.mp4"
        ])
        .stdin(Stdio::piped())
        .spawn()
        .expect("Не удалось открять ffmpeg!");

    let mut ffmpeg_stdin = ffmpeg.stdin.take().expect("Нет доступа к stdin ffmpeg");

    let duration = Duration::from_secs(10);
    let start = Instant::now();

    while start.elapsed() < duration {
        match capturer.frame() {
            Ok(frame) => {
                ffmpeg_stdin.write_all(&frame)?;
            }
            Err(ref e) => if e.kind() == WouldBlock {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
    }

    println!("Останавливаем запись!");
    drop(ffmpeg_stdin);
    ffmpeg.wait()?;

    println!("Видео сохранено: output.mp4");
    Ok(())
}

pub fn measure_fps(duration_secs: u64) -> Result<f64, Box<dyn std::error::Error>> {
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?;
    let start = Instant::now();
    let mut frames = 0;
    let (width, height) = (capturer.width(), capturer.height());
    while start.elapsed().as_secs() < duration_secs {
        match capturer.frame() {
            Ok(frame) => {
                let mut rgba_buf = vec![0u8; width * height * 4];
                for (i, chunk) in frame.chunks(4).enumerate() {
                    let j = i * 4;
                    rgba_buf[j] = chunk[2];     // R
                    rgba_buf[j + 1] = chunk[1]; // G
                    rgba_buf[j + 2] = chunk[0]; // B
                    rgba_buf[j + 3] = 255;      // A
                }
                frames += 1;
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    let fps = frames as f64 / duration_secs as f64;
    Ok(fps)
}