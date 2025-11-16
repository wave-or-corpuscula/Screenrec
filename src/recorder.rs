use ctrlc;

use nix::unistd::Pid;
use nix::sys::signal::{kill, Signal};

use scrap::{Display, Capturer};
use std::error::Error;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{io::{ErrorKind::WouldBlock, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};




pub struct RecorderConfig {
    pub output: String,
    pub fps: u32,
    pub audio_source: String
}

impl RecorderConfig {
    pub fn new(output: String, fps: u32, audio_source: Option<String>) -> Self {
        Self {
            output,
            fps,
            audio_source: audio_source.unwrap_or_else(|| "default".to_string())
        }
    }
}



pub struct VideoCapturer {
    capturer: Capturer,
    width: usize,
    height: usize
}


impl VideoCapturer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::primary()?;
        let capturer = Capturer::new(display)?;

        let width = capturer.width();
        let height = capturer.height();

        println!("Screen resolution {}x{}", width, height);

        Ok(Self {
            capturer,
            width,
            height
        })
    }

    pub fn frame(&mut self) -> Option<Vec<u8>> {
        match self.capturer.frame() {
            Ok(frame) => Some(frame.to_vec()),
            Err(e) if e.kind() == WouldBlock => None,
            Err(e) => {
                eprintln!("Error capture with error: {}", e);
                None
            }
        }
    }
}



pub struct FfmpegProcess {
    child: Child,
    stdin: Option<std::process::ChildStdin>
}

impl FfmpegProcess {
    pub fn start(config: &RecorderConfig, width: usize, height: usize) -> Result<Self, Box<dyn Error>> {
        let mut cmd = Command::new("ffmpeg");

        let args = vec![
            "-y".to_string(),
            "-init_hw_device".to_string(), "vaapi=va:/dev/dri/renderD128".to_string(),
            "-filter_hw_device".to_string(), "va".to_string(),
            "-f".to_string(), "rawvideo".to_string(),
            "-pixel_format".to_string(), "bgr0".to_string(),
            "-video_size".to_string(), format!("{}x{}", width, height),
            "-framerate".to_string(), format!("{}", config.fps),
            "-i".to_string(), "-".to_string(),
            "-f".to_string(), "pulse".to_string(),
            "-i".to_string(), config.audio_source.clone(),
            "-vf".to_string(), "format=nv12,hwupload=extra_hw_frames=16".to_string(),
            "-c:v".to_string(), "h264_vaapi".to_string(),
            "-qp".to_string(), "23".to_string(),
            "-c:a".to_string(), "aac".to_string(),
            "-b:a".to_string(), "192k".to_string(),
            config.output.clone(),
        ];

        let mut child = cmd.args(&args).stdin(Stdio::piped()).spawn()?;
        println!("Ffmpeg started with pid: {}", child.id());

        let stdin = child.stdin.take();

        Ok (Self {child, stdin})
    }

    pub fn send_frame(&mut self, frame: &[u8]) -> bool {
        if let Some(stdin) = self.stdin.as_mut() {
            if stdin.write_all(frame).is_err() {
                return false
            }
            true
        } else {
            false
        }
    } 
    pub fn stop(mut self) {
        drop(self.stdin.take());

        let pid = self.child.id() as i32;
        let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);

        for _ in 0..40 {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                _ => thread::sleep(Duration::from_millis(100))
            }
        }

        let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
        let _ = self.child.wait();
    }
}



pub struct ScreenRecorder {
    config: RecorderConfig,
    video: VideoCapturer,
    ffmpeg: Option<FfmpegProcess>,
    stop_flag: Arc<AtomicBool>
}

impl ScreenRecorder {
    pub fn new(config: RecorderConfig) -> Result<Self, Box<dyn Error>> {
        Ok (Self {
            config,
            video: VideoCapturer::new()?,
            ffmpeg: None,
            stop_flag: Arc::new(AtomicBool::new(false))
        })
    }

    
    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let (w, h) = (self.video.width, self.video.height);
        
        self.ffmpeg = Some(FfmpegProcess::start(&self.config, w, h)?);
        
        self.init_ctrlc_handler();

        println!("Запись началась, нажмите Ctrl + C для остановки");

        self.record_loop();

        Ok(())
    }

    pub fn record_loop(&mut self) {
        let frame_interval = Duration::from_micros(1_000_000 / self.config.fps as u64);
        let mut last_frame_time = Instant::now();

        while !self.stop_flag.load(Ordering::SeqCst) { // je no compredre pas
            let now = Instant::now();
            if now - last_frame_time < frame_interval {
                thread::sleep(frame_interval - (now - last_frame_time));
            }
            last_frame_time = Instant::now();

            if let Some(frame) = self.video.frame() {
                if !self.ffmpeg.as_mut().unwrap().send_frame(&frame) {
                    eprintln!("Ffmpeg закрыт, завершаем цикл!");
                    break;
                }
            }
        }

        println!("Завершаем запись!");
        self.ffmpeg.take().unwrap().stop();
    }
    
    pub fn init_ctrlc_handler(&self) {
        let flag = self.stop_flag.clone();
        ctrlc::set_handler(move || {
            flag.store(true, Ordering::SeqCst);
            eprintln!("Ctrl + C signal received");
        })
        .expect("Ctrl + C signal error!")

    }

}






// pub fn record_screen() -> Result<(), Box<dyn Error>> {
//     // --- Параметры ---
//     // Можно менять
//     let out_path = "output/output.mp4";
//     // если хочешь, подставь конкретное имя мониторного источника PulseAudio,
//     // например "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor".
//     // По умолчанию используем "default" (может быть перенаправлен системно на .monitor)
//     let audio_source = std::env::var("AUDIO_SOURCE").unwrap_or_else(|_| "default".to_string());
//     // частота вывода кадров (как хочешь)
//     let target_fps: u32 = 30;

//     // --- Инициализация scrap ---
//     let display = Display::primary()?;
//     let mut capturer = Capturer::new(display)?;
//     let (width, height) = (capturer.width(), capturer.height());
//     println!("Screen size: {}x{}", width, height);

//     // --- Обработка Ctrl+C: только ставим флаг ---
//     let stop_flag = Arc::new(AtomicBool::new(false));
//     {
//         let stop_flag_clone = stop_flag.clone();
//         ctrlc::set_handler(move || {
//             // Сигнал пришёл — ставим флаг. Всё остальное сделает основной поток.
//             stop_flag_clone.store(true, Ordering::SeqCst);
//             eprintln!("Ctrl+C получен — начинаем корректное завершение...");
//         }).expect("Не удалось установить Ctrl-C handler");
//     }

//     // --- Запуск ffmpeg ---
//     // Мы будем передавать кадры в stdin ffmpeg (rawvideo bgr0).
//     // ffmpeg сам откроет PulseAudio источник (audio_source).
//     let mut ffmpeg_cmd = Command::new("ffmpeg");

//     // Формируем аргументы как Vec<String>, т.к. размеры — runtime
//     let args = vec![
//         "-y".to_string(),
//         // инициализируем vaapi device (на Intel)
//         "-init_hw_device".to_string(), "vaapi=va:/dev/dri/renderD128".to_string(),
//         "-filter_hw_device".to_string(), "va".to_string(),

//         // видео вход (stdin)
//         "-f".to_string(), "rawvideo".to_string(),
//         "-pixel_format".to_string(), "bgr0".to_string(),
//         "-video_size".to_string(), format!("{}x{}", width, height),
//         "-framerate".to_string(), format!("{}", target_fps),
//         "-i".to_string(), "-".to_string(),

//         // аудио вход (PulseAudio). Заменяй audio_source, если нужно.
//         "-f".to_string(), "pulse".to_string(),
//         "-i".to_string(), audio_source.clone(),

//         // фильтры и hwupload
//         "-vf".to_string(), "format=nv12,hwupload=extra_hw_frames=16".to_string(),

//         // видеокодек на VAAPI (Intel)
//         "-c:v".to_string(), "h264_vaapi".to_string(),
//         "-qp".to_string(), "23".to_string(),

//         // аудиокодек
//         "-c:a".to_string(), "aac".to_string(),
//         "-b:a".to_string(), "192k".to_string(),

//         out_path.to_string(),
//     ];

//     // Запускаем
//     let mut child = ffmpeg_cmd
//         .args(&args)
//         .stdin(Stdio::piped())
//         .spawn()
//         .expect("Не удалось запустить ffmpeg. Проверь путь и аргументы.");

//     println!("Запущен ffmpeg (pid {}).", child.id());
//     println!("FFmpeg args: {:?}", args);

//     // Берём stdin ffmpeg
//     let mut ffmpeg_stdin = child.stdin.take().expect("stdin ffmpeg оказался None");

//     // Frame limiter
//     let frame_duration = Duration::from_micros(1_000_000u64 / target_fps as u64);
//     let mut last_frame_time = Instant::now();

//     // Счётчик FPS для отладки
//     let mut frames_sent: u64 = 0;
//     let mut fps_print_time = Instant::now();

//     println!("Запись началась — нажмите Ctrl+C для остановки.");

//     // --- Основной цикл: пока пользователь не нажал Ctrl+C ---
//     while !stop_flag.load(Ordering::SeqCst) {
//         // Стабилизация интервала (чтобы ffmpeg получал кадры ровно с target_fps)
//         let now = Instant::now();
//         if now - last_frame_time < frame_duration {
//             thread::sleep(frame_duration - (now - last_frame_time));
//         }
//         last_frame_time = Instant::now();

//         match capturer.frame() {
//             Ok(frame) => {
//                 // frame — &[u8] в формате B G R X (bgr0)
//                 // Записываем "как есть" — ffmpeg сделает конвертацию format=nv12,hwupload
//                 if let Err(e) = ffmpeg_stdin.write_all(&frame) {
//                     eprintln!("Ошибка записи в stdin ffmpeg: {}", e);
//                     // Если BrokenPipe — process, вероятно, умер. Прекращаем цикл.
//                     break;
//                 }
//                 frames_sent += 1;
//             }
//             Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
//                 // кадр пока не готов — просто продолжаем (следующая итерация)
//                 continue;
//             }
//             Err(e) => {
//                 eprintln!("Ошибка захвата кадра: {}", e);
//                 break;
//             }
//         }

//         // Печатаем FPS раз в 2 секунды (по желанию)
//         if fps_print_time.elapsed() >= Duration::from_secs(2) {
//             let fps = frames_sent as f64 / fps_print_time.elapsed().as_secs_f64();
//             println!("Отправлено кадров: {}, ~FPS: {:.2}", frames_sent, fps);
//             // сбросим счётчик и таймер
//             frames_sent = 0;
//             fps_print_time = Instant::now();
//         }
//     }

//     // --- Начинаем корректное завершение ffmpeg ---
//     eprintln!("Начинаем корректное завершение ffmpeg...");

//     // 1) Закрываем stdin — видео-поток закончился (EOF)
//     drop(ffmpeg_stdin);

//     // 2) Отправляем SIGTERM — мягкое завершение (чтобы ffmpeg остановил аудио-вход и дописал файл)
//     let pid = child.id() as i32;
//     let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);

//     // 3) Ждём завершения процесса с таймаутом (например, 8 секунд).
//     let mut waited = 0u32;
//     loop {
//         match child.try_wait() {
//             Ok(Some(status)) => {
//                 println!("ffmpeg завершился с статусом: {}", status);
//                 break;
//             }
//             Ok(None) => {
//                 // ещё жив — подождём немного
//                 if waited >= 80 { // 80 * 100ms = 8s
//                     eprintln!("ffmpeg не завершился за 8s — посылаем SIGKILL");
//                     let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
//                     // даём ещё секунду и выходим
//                     thread::sleep(Duration::from_secs(1));
//                     let _ = child.wait();
//                     break;
//                 } else {
//                     waited += 1;
//                     thread::sleep(Duration::from_millis(100));
//                 }
//             }
//             Err(e) => {
//                 eprintln!("Ошибка проверки состояния ffmpeg: {}", e);
//                 break;
//             }
//         }
//     }

//     println!("Готово. Видео сохранено в {}", out_path);
//     Ok(())
// }


// pub fn measure_fps(duration_secs: u64) -> Result<f64, Box<dyn std::error::Error>> {
//     let display = Display::primary()?;
//     let mut capturer = Capturer::new(display)?;
//     let start = Instant::now();
//     let mut frames = 0;
//     let (width, height) = (capturer.width(), capturer.height());
//     while start.elapsed().as_secs() < duration_secs {
//         match capturer.frame() {
//             Ok(frame) => {
//                 let mut rgba_buf = vec![0u8; width * height * 4];
//                 for (i, chunk) in frame.chunks(4).enumerate() {
//                     let j = i * 4;
//                     rgba_buf[j] = chunk[2];     // R
//                     rgba_buf[j + 1] = chunk[1]; // G
//                     rgba_buf[j + 2] = chunk[0]; // B
//                     rgba_buf[j + 3] = 255;      // A
//                 }
//                 frames += 1;
//             },
//             Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
//                 thread::sleep(Duration::from_millis(5));
//             }
//             Err(e) => return Err(Box::new(e)),
//         }
//     }

//     let fps = frames as f64 / duration_secs as f64;
//     Ok(fps)
// }

// todo: OOP here