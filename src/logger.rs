use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::Instant;

/// Global channel to the background logger thread.
/// Sending a message is near-instant (just pushes to a queue);
/// the actual file I/O happens on the background thread.
static LOG_SENDER: OnceLock<Sender<String>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Returns the log file path: %APPDATA%\LightSwitch\lightswitch.log
pub fn log_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata)
        .join("LightSwitch")
        .join("lightswitch.log")
}

/// Initializes the logger: opens the log file (truncating any existing one)
/// and spawns a background thread that drains incoming log messages.
/// Should be called once at program startup.
pub fn init() {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[logger] Failed to open log file {:?}: {}", path, e);
            return;
        }
    };

    let (sender, receiver) = channel::<String>();
    let _ = LOG_SENDER.set(sender);
    let _ = START_TIME.set(Instant::now());

    // Background thread: drains the channel and writes to the file.
    std::thread::Builder::new()
        .name("lightswitch-logger".to_string())
        .spawn(move || {
            let mut file = file;
            let _ = writeln!(
                file,
                "[      0ms] === LightSwitch v{} (built {}) log started ===",
                env!("CARGO_PKG_VERSION"),
                env!("BUILD_TIMESTAMP")
            );
            let _ = file.flush();

            while let Ok(msg) = receiver.recv() {
                let _ = writeln!(file, "{}", msg);
                // Flush only occasionally to reduce I/O pressure; the channel
                // buffers messages for us so we don't lose them on crashes.
                let _ = file.flush();
            }
        })
        .expect("failed to spawn logger thread");
}

/// Enqueues a log message. This call is non-blocking and fast — it only
/// pushes a formatted string into the background channel. Safe to call
/// from the keyboard hook callback without exceeding Windows' 300ms timeout.
pub fn log(msg: &str) {
    let elapsed = START_TIME
        .get()
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0);
    let formatted = format!("[{:>7}ms] {}", elapsed, msg);

    if let Some(sender) = LOG_SENDER.get() {
        let _ = sender.send(formatted);
    } else {
        // Fallback if logger wasn't initialized (shouldn't happen in practice)
        eprintln!("{}", msg);
    }
}

/// Convenience macro for logging formatted messages.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}
