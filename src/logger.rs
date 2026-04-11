use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
static START_TIME: Mutex<Option<Instant>> = Mutex::new(None);

/// Returns the log file path: %APPDATA%\LightSwitch\lightswitch.log
pub fn log_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata)
        .join("LightSwitch")
        .join("lightswitch.log")
}

/// Initializes the logger: creates the log file (truncating any existing one).
/// Should be called once at program startup.
pub fn init() {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        Ok(file) => {
            *LOG_FILE.lock().unwrap() = Some(file);
            *START_TIME.lock().unwrap() = Some(Instant::now());
            log("=== LightSwitch log started ===");
        }
        Err(e) => {
            eprintln!("[logger] Failed to open log file {:?}: {}", path, e);
        }
    }
}

/// Writes a line to both stderr and the log file.
pub fn log(msg: &str) {
    eprintln!("{}", msg);

    let mut guard = LOG_FILE.lock().unwrap();
    if let Some(file) = guard.as_mut() {
        let elapsed = START_TIME
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "[{:>7}ms] {}", elapsed, msg);
        let _ = file.flush();
    }
}

/// Convenience macro for logging formatted messages.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}
