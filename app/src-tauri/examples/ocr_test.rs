#[path = "../src/platform/mod.rs"]
mod platform;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ocr_test <image>");
    let png = std::fs::read(&path).expect("read image");
    match platform::ocr_png(&png) {
        Ok(lines) => {
            println!("识别到 {} 行:", lines.len());
            for l in lines {
                println!("  | {l}");
            }
        }
        Err(e) => {
            eprintln!("OCR 失败: {e}");
            std::process::exit(1);
        }
    }
}
