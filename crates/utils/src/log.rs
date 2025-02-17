use std::{env, time::Duration};

use tracing_subscriber::EnvFilter;

pub fn init_tracing_logger() {
    let rust_log = env::var(EnvFilter::DEFAULT_ENV).unwrap_or_default();
    let env_filter = match rust_log.is_empty() {
        true => EnvFilter::builder().parse_lossy("info,discv5=error,utp_rs=error"),
        false => EnvFilter::builder().parse_lossy(rust_log),
    };

    match env::var("TRACING_CONSOLE_PORT") {
        Ok(port) => {
            let port = port.parse::<u16>().unwrap();
            console_subscriber::ConsoleLayer::builder()
                // set how long the console will retain data from completed tasks
                .retention(Duration::from_secs(60))
                // set the address the server is bound to
                .server_addr(([127, 0, 0, 1], port))
                .init();
        }
        Err(_) => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_ansi(detect_ansi_support())
                .init();
        }
    }
}

pub fn detect_ansi_support() -> bool {
    #[cfg(windows)]
    {
        use ansi_term::enable_ansi_support;
        enable_ansi_support().is_ok()
    }
    #[cfg(not(windows))]
    {
        // Detect whether our log output (which goes to stdout) is going to a terminal.
        // For example, instead of the terminal, it might be getting piped into another file, which
        // probably ought to be plain text.
        let is_terminal = atty::is(atty::Stream::Stdout);
        if !is_terminal {
            return false;
        }

        // Return whether terminal defined in TERM supports ANSI
        std::env::var("TERM")
            .map(|term| term != "dumb")
            .unwrap_or(false)
    }
}
