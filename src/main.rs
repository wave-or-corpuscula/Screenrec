mod recorder;
use recorder::{RecorderConfig, ScreenRecorder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RecorderConfig::new("output/output.mp4".into(), 30, None);

    let mut recorder = ScreenRecorder::new(config)?;
    recorder.start()?;

    Ok(())
}

/*
todo:
    scrap - для записи экрана (+ звук внутри компьютера)
    cpal - для записи микрофона

*/
