mod recorder;
use recorder::{record_screen, measure_fps};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // match measure_fps(10) {
    //     Ok(fps) => println!("Средний FPS: {:.2}", fps),
    //     Err(e) => eprintln!("Ошибка: {}", e),
    // }

    record_screen()?;
    Ok(())
}

/*
todo:
    scrap - для записи экрана (+ звук внутри компьютера)
    cpal - для записи микрофона

*/
