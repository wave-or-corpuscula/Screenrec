use image::{ImageBuffer, Rgba};
use scrap::{Display, Capturer};
use std::{io::ErrorKind::WouldBlock, thread, time::Duration};


// Говорим, что если все пойдет нормально, то мы вернем (), а
// если нет, то любую ошибку реализующую интерфейс Error
// Это нужно, чтобы удобно обрабатывать ошибки с помощью ?
fn main() -> Result<(), Box<dyn std::error::Error>> {

    let display = Display::primary()?; // Синтаксический сахар
    let mut capturer = Capturer::new(display)?;

    let (width, height) = (capturer.width(), capturer.height());
    println!("Размеры экрана {}x{}", width, height);

    // Попытка получнеия кадра
    let frame = loop {
        match capturer.frame() {
            Ok(buffer) => break buffer.to_vec(),
            Err(ref e) => if e.kind() == WouldBlock { // Ошибка буфера (он пока не готов)
                println!("Failed to capture, retry");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        }
    };

    let mut rgba_buf = Vec::with_capacity(width * height * 4);
    for chunk in frame.chunks(4) { // Дает нам формат (BGRA)
        rgba_buf.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]); // Переделываем в (RGBA)
    }

    let img: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(width as u32, height as u32, rgba_buf)
        .expect("No image buffer(((");

    img.save("screenshot.png")?;

    println!("Screenshot is saved!");
    Ok(())
}
