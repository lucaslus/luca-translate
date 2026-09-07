//! Opt-in, finite diagnostic executed with the installed app's own TCC identity.
//! Captures only the explicitly supplied rectangle. Logs metadata, not content;
//! never requests/resets permission, writes the clipboard, or calls translation.
use crate::platform::{self, Region};
use serde_json::json;
use std::time::Instant;

pub fn run_if_requested() -> Option<Result<(), String>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--diagnose-ocr") {
        return None;
    }
    Some((|| {
        let region = parse_region(&args.next().ok_or("需要 x,y,w,h 参数")?)?;
        if args.next().is_some() {
            return Err("用法: --diagnose-ocr x,y,w,h".into());
        }
        println!(
            "{}",
            json!({
                "executable": std::env::current_exe().ok(),
                "main_thread_permission": platform::screen_capture_available(),
                "region": region,
            })
        );
        std::thread::spawn(move || {
            for round in 1..=3 {
                let allowed = platform::screen_capture_available();
                if !allowed {
                    println!("{}", json!({"round": round, "permission": false}));
                    return Err("当前应用进程没有屏幕录制权限".into());
                }
                let started = Instant::now();
                let png = platform::capture_screen_region(&region)?;
                let capture_ms = started.elapsed().as_millis();
                let started = Instant::now();
                let lines = platform::ocr_png(&png)?;
                println!(
                    "{}",
                    json!({
                        "round": round,
                        "permission_before": allowed,
                        "permission_after": platform::screen_capture_available(),
                        "capture_ms": capture_ms,
                        "ocr_ms": started.elapsed().as_millis(),
                        "lines": lines.len(),
                        "characters": lines.iter().map(|s| s.chars().count()).sum::<usize>(),
                    })
                );
            }
            Ok(())
        })
        .join()
        .map_err(|_| "OCR 诊断线程异常退出")?
    })())
}

fn parse_region(value: &str) -> Result<Region, String> {
    let values = value
        .split(',')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "截图坐标必须是整数")?;
    match values.as_slice() {
        &[x, y, w, h] if w > 0 && h > 0 => Ok(Region { x, y, w, h }),
        _ => Err("需要 x,y,w,h，宽高必须大于 0".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn region_requires_explicit_nonempty_rectangle() {
        for invalid in ["", "1,2,3", "1,2,0,10", "1,2,-3,4", "a,2,3,4", "1,2,3,4,5"] {
            assert!(parse_region(invalid).is_err());
        }
        assert_eq!(parse_region("-100,20,300,40").unwrap().x, -100);
    }
}
