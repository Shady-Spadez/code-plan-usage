use std::path::PathBuf;

pub fn init_logger() {
    // Logger is initialized via the debug_log! macro which writes
    // directly to the log file. This function exists for explicit
    // initialization if needed in the future.
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .expect("无法获取当前可执行文件路径")
        .parent()
        .expect("无法获取可执行文件父目录")
        .to_path_buf()
}

pub fn log_path() -> PathBuf {
    exe_dir().join("coding_plan_widget.log")
}

/// Write a timestamped message to the log file next to the exe.
/// In debug builds, also prints to stderr (visible in the console window).
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let timestamp = chrono::Local::now().format("%H:%M:%S");
        let line = format!("[{}] {}\n", timestamp, msg);
        // Always write to log file
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open($crate::log::log_path())
        {
            let _ = std::io::Write::write_all(&mut file, line.as_bytes());
            let _ = std::io::Write::flush(&mut file);
        }
        // In debug builds, also print to stderr (console)
        #[cfg(debug_assertions)]
        {
            let console_line = format!("[{}] {}", timestamp, msg);
            eprintln!("{}", console_line);
        }
    }};
}
