//! Build script: bakes the current timestamp into the binary so the runtime
//! log can identify which build is running.

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    println!("cargo:rustc-env=BUILD_UNIX_TS={}", secs);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", format_utc(secs));

    // Embed the application icon in the EXE (Windows only). Silently skips if
    // assets/icon.ico is missing so the build still works before the icon is
    // generated (run `cargo run --example gen_icon` to create it).
    #[cfg(target_os = "windows")]
    {
        let icon_path = std::path::Path::new("assets/icon.ico");
        if icon_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=Failed to embed icon: {}", e);
            }
            println!("cargo:rerun-if-changed=assets/icon.ico");
        } else {
            println!("cargo:warning=assets/icon.ico not found — EXE will have no icon");
        }
    }
}

/// Formats a unix timestamp as "YYYY-MM-DD HH:MM:SS UTC" (no deps).
fn format_utc(ts: u64) -> String {
    let sec = ts % 60;
    let min = (ts / 60) % 60;
    let hour = (ts / 3600) % 24;
    let mut days = ts / 86400;

    let mut year: u64 = 1970;
    loop {
        let yd = if is_leap(year) { 366 } else { 365 };
        if days < yd {
            break;
        }
        days -= yd;
        year += 1;
    }

    let months = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month: u64 = 1;
    let mut day = days + 1;
    for &m in &months {
        if day <= m {
            break;
        }
        day -= m;
        month += 1;
    }

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02} UTC")
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
