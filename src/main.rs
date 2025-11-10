use scrap::{Display, Capturer};
use std::{io::{ErrorKind::WouldBlock, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};

// Говорим, что если все пойдет нормально, то мы вернем (), а
// если нет, то любую ошибку реализующую интерфейс Error
// Это нужно, чтобы удобно обрабатывать ошибки с помощью ?
fn main() -> Result<(), Box<dyn std::error::Error>> {

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
            "-f", "rawvideo",
            "-pixel_format", "rgba",
            "-video_size", &format!("{width}x{height}"),
            "-framerate", "30",
            "-i", "-", // stdin
            "-c:v", "libx264", // h264_nvenc
            // "-preset", "fast",
             "-pix_fmt", "yuv420p",
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
                let mut rgba_buf = Vec::with_capacity(width * height * 4);
                for chunk in frame.chunks(4) {
                    rgba_buf.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
                }
                ffmpeg_stdin.write_all(&rgba_buf)?;
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
