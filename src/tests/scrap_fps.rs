use scrap::{Display, Capturer};
use std::{io::{ErrorKind::WouldBlock, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};


fn main() {
    match measure_fps(5) {
        Ok(fps) => println!("Средний FPS: {:.2}", fps),
        Err(e) => eprintln!("Ошибка: {}", e),
    }
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