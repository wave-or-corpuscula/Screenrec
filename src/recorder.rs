use ctrlc;
use scrap::{Display, Capturer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{io::ErrorKind::WouldBlock, thread, time::{Duration, Instant}};
use anyhow::Result;
use ffmpeg_next as ffmpeg;

pub struct RecorderConfig {
    pub output: String,
    pub fps: u32,
    pub audio_source: String,
    pub video_quality: u32,
}

impl RecorderConfig {
    pub fn new(output: String, fps: u32, audio_source: Option<String>) -> Self {
        Self {
            output,
            fps,
            audio_source: audio_source.unwrap_or_else(|| "default".to_string()),
            video_quality: 23, // CRF для H.264
        }
    }

    pub fn with_quality(mut self, quality: u32) -> Self {
        self.video_quality = quality;
        self
    }
}

pub struct VideoCapturer {
    capturer: Capturer,
    width: u32,
    height: u32,
}

impl VideoCapturer {
    pub fn new() -> Result<Self> {
        let display = Display::primary()?;
        let capturer = Capturer::new(display)?;

        let width = capturer.width() as u32;
        let height = capturer.height() as u32;

        println!("📺 Screen resolution: {}x{}", width, height);

        Ok(Self {
            capturer,
            width,
            height,
        })
    }

    pub fn frame(&mut self) -> Option<Vec<u8>> {
        match self.capturer.frame() {
            Ok(frame) => Some(frame.to_vec()),
            Err(e) if e.kind() == WouldBlock => None,
            Err(e) => {
                eprintln!("❌ Frame capture error: {}", e);
                None
            }
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

// Конвертер кадров из BGR0 в RGB24 для FFmpeg
struct FrameConverter {
    width: u32,
    height: u32,
    rgb_buffer: Vec<u8>,
}

impl FrameConverter {
    fn new(width: u32, height: u32) -> Self {
        let rgb_buffer = vec![0u8; (width * height * 3) as usize];

        Self {
            width,
            height,
            rgb_buffer,
        }
    }

    fn convert_bgr0_to_rgb24(&mut self, bgr0_frame: &[u8]) -> &[u8] {
        let pixel_count = (self.width * self.height) as usize;

        for i in 0..pixel_count {
            let src_idx = i * 4; // BGR0 = 4 байта на пиксель
            let dst_idx = i * 3; // RGB24 = 3 байта на пиксель

            if src_idx + 2 < bgr0_frame.len() {
                // BGR0 -> RGB
                self.rgb_buffer[dst_idx] = bgr0_frame[src_idx + 2];     // R
                self.rgb_buffer[dst_idx + 1] = bgr0_frame[src_idx + 1]; // G
                self.rgb_buffer[dst_idx + 2] = bgr0_frame[src_idx];     // B
            }
        }

        &self.rgb_buffer
    }
}

// Основной класс для работы с FFmpeg
pub struct FfmpegEncoder {
    converter: FrameConverter,
    encoder: ffmpeg::encoder::Video,
    format: ffmpeg::format::Output,
    frame: ffmpeg::frame::Video,
    packet: ffmpeg::Packet,
    time_base: ffmpeg::Rational,
    frame_count: i64,
    stream_index: usize,
}

impl FfmpegEncoder {
    pub fn new(config: &RecorderConfig, width: u32, height: u32) -> Result<Self> {
        // Инициализация FFmpeg
        ffmpeg::init()?;

        // Создаем output format
        let mut oformat = ffmpeg::format::output(&config.output)?;

        // Находим H.264 кодек
        let codec = ffmpeg::encoder::find_by_name("libx264")
            .expect("H.264 codec not found");

        // Настраиваем кодек через builder pattern
        let mut encoder = codec.video().expect("Failed to create video encoder");
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_pix_fmt(ffmpeg::format::Pixel::YUV420P);
        encoder.set_gop(30);
        encoder.set_time_base(ffmpeg::Rational::new(1, config.fps as i32));
        encoder.set_frame_rate(Some(ffmpeg::Rational::new(config.fps as i32, 1)));
        encoder.set_bit_rate(2_000_000);

        // Открываем кодек
        let encoder = encoder.open_with(None)?;

        // Создаем stream
        let stream = oformat.add_stream(encoder.codec())?;
        stream.set_parameters(encoder.parameters());
        let stream_index = stream.index();

        // Записываем заголовок
        oformat.write_header()?;

        // Создаем фрейм для конвертации
        let frame = ffmpeg::frame::Video::new(
            ffmpeg::format::Pixel::RGB24,
            width,
            height
        );

        // Создаем пакет для кодированных данных
        let packet = ffmpeg::Packet::empty();

        let time_base = stream.time_base();

        println!("✅ FFmpeg encoder initialized successfully");
        println!("   Output: {}", config.output);
        println!("   Resolution: {}x{}", width, height);
        println!("   FPS: {}", config.fps);
        println!("   Quality: CRF {}", config.video_quality);

        Ok(Self {
            converter: FrameConverter::new(width, height),
            encoder,
            format: oformat,
            frame,
            packet,
            time_base,
            frame_count: 0,
            stream_index,
        })
    }

    pub fn send_frame(&mut self, bgr0_frame: &[u8]) -> Result<()> {
        // Конвертируем BGR0 в RGB24
        let rgb_data = self.converter.convert_bgr0_to_rgb24(bgr0_frame);

        // Копируем данные во фрейм FFmpeg
        self.frame.data_mut(0).copy_from_slice(rgb_data);

        // Устанавливаем PTS (Presentation Time Stamp)
        self.frame.set_pts(Some(self.frame_count));
        self.frame_count += 1;

        // Отправляем фрейм в кодер
        self.encoder.send_frame(&self.frame)?;

        // Получаем закодированные пакеты
        while self.encoder.receive_packet(&mut self.packet).is_ok() {
            self.packet.rescale_ts(
                self.encoder.time_base(),
                self.time_base
            );
            self.packet.set_stream(self.stream_index);
            self.format.write_packet(&self.packet)?;
        }

        Ok(())
    }

    // Финализация записи
    pub fn finish(mut self) -> Result<()> {
        println!("🔄 Finalizing video...");

        // Отправляем EOF в кодер
        self.encoder.send_eof()?;

        // Получаем оставшиеся пакеты
        while self.encoder.receive_packet(&mut self.packet).is_ok() {
            self.packet.rescale_ts(
                self.encoder.time_base(),
                self.time_base
            );
            self.packet.set_stream(self.stream_index);
            self.format.write_packet(&self.packet)?;
        }

        // Записываем trailer - финализация MP4 файла
        self.format.write_trailer()?;

        println!("✅ Video successfully saved!");
        Ok(())
    }
}

// Drop trait для автоматической очистки
impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        let _ = self.encoder.send_eof();

        while self.encoder.receive_packet(&mut self.packet).is_ok() {
            self.packet.rescale_ts(
                self.encoder.time_base(),
                self.time_base
            );
            self.packet.set_stream(self.stream_index);
            let _ = self.format.write_packet(&self.packet);
        }

        let _ = self.format.write_trailer();
    }
}

pub struct ScreenRecorder {
    config: RecorderConfig,
    video: VideoCapturer,
    stop_flag: Arc<AtomicBool>,
}

impl ScreenRecorder {
    pub fn new(config: RecorderConfig) -> Result<Self> {
        Ok(Self {
            video: VideoCapturer::new()?,
            config,
            stop_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let (width, height) = self.video.dimensions();

        // Создаем энкодер
        let encoder = FfmpegEncoder::new(&self.config, width, height)?;

        self.init_ctrlc_handler();
        self.print_recording_info();

        self.record_loop(encoder)?;

        Ok(())
    }

    fn print_recording_info(&self) {
        println!("🎥 Screen Recorder Started");
        println!("═══════════════════════════════════════");
        println!("📹 Output file: {}", self.config.output);
        println!("🎯 Target FPS: {}", self.config.fps);
        println!("🎵 Audio source: {}", self.config.audio_source);
        println!("📊 Quality: CRF {}", self.config.video_quality);
        println!("═══════════════════════════════════════");
        println!("\n📢 Press Ctrl+C to stop recording\n");
    }

    pub fn record_loop(&mut self, mut encoder: FfmpegEncoder) -> Result<()> {
        let frame_interval = Duration::from_micros(1_000_000 / self.config.fps as u64);
        let mut last_frame_time = Instant::now();

        let mut frames_processed = 0u64;
        let mut fps_report_time = Instant::now();

        while !self.stop_flag.load(Ordering::SeqCst) {
            // Стабилизация FPS
            let now = Instant::now();
            if now - last_frame_time < frame_interval {
                thread::sleep(frame_interval - (now - last_frame_time));
            }
            last_frame_time = Instant::now();

            // Получаем и обрабатываем кадр
            if let Some(frame) = self.video.frame() {
                if let Err(e) = encoder.send_frame(&frame) {
                    eprintln!("❌ Failed to encode frame: {}", e);
                    break;
                }
                frames_processed += 1;
            }

            // Отчет о FPS каждые 5 секунд
            if fps_report_time.elapsed() >= Duration::from_secs(5) {
                let actual_fps = frames_processed as f64 / fps_report_time.elapsed().as_secs_f64();
                println!("📊 Processed: {} frames | {:.2} FPS", frames_processed, actual_fps);
                frames_processed = 0;
                fps_report_time = Instant::now();
            }
        }

        println!("\n🛑 Recording stopped, finalizing video...");

        // Финализация записи
        encoder.finish()?;

        Ok(())
    }

    fn init_ctrlc_handler(&self) {
        let flag = self.stop_flag.clone();
        ctrlc::set_handler(move || {
            flag.store(true, Ordering::SeqCst);
            println!("\n⚠️  Ctrl+C received! Gracefully stopping...");
        })
        .expect("Failed to set Ctrl+C handler");
    }
}

// Удобная функция для быстрого старта
pub fn quick_record(output: &str, fps: u32) -> Result<()> {
    let config = RecorderConfig::new(output.to_string(), fps, None)
        .with_quality(23); // Среднее качество

    let mut recorder = ScreenRecorder::new(config)?;
    recorder.start()
}

// Функция для записи с высоким качеством
pub fn high_quality_record(output: &str, fps: u32) -> Result<()> {
    let config = RecorderConfig::new(output.to_string(), fps, None)
        .with_quality(18); // Высокое качество (меньше = лучше)

    let mut recorder = ScreenRecorder::new(config)?;
    recorder.start()
}