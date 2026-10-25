//! Manual check of selection capture against whatever app is frontmost.
//! `cargo run --example axcap -- [delay_secs]` from a terminal that has Accessibility permission.
use speech_hub_lib::accessibility::{Target, WARMUP_BUDGET};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    // `axcap <pid>` targets that process; `axcap` alone targets the frontmost app.
    let pid: Option<i32> = std::env::args().nth(1).and_then(|s| s.parse().ok());
    let t0 = Instant::now();
    let target = match pid {
        Some(pid) => Target::for_pid(pid),
        None => match Target::frontmost() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("no target: {e}");
                std::process::exit(1);
            }
        },
    };
    println!("target: {}", target.app_name.clone().unwrap_or_default());
    let cancel = AtomicBool::new(false);
    match target.capture(WARMUP_BUDGET, &cancel, |name| println!("slow path for {name:?} at {} ms", t0.elapsed().as_millis())) {
        Ok(sel) => println!("ok after {} ms ({} chars): {}", t0.elapsed().as_millis(), sel.text.chars().count(), sel.text.chars().take(80).collect::<String>()),
        Err(e) => {
            println!("failed after {} ms: {e}", t0.elapsed().as_millis());
            std::process::exit(2);
        }
    }
}
