use clap::ValueEnum;
use std::path::Path;
use std::{fmt::Display, io};
use tracing::subscriber::SetGlobalDefaultError;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::Layered;
use tracing_subscriber::{
    filter::EnvFilter, filter::LevelFilter, fmt::Layer, layer::SubscriberExt, reload, Registry,
};

/// Our set of supported log levels.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogLevel {
    #[clap(name = "OFF")]
    Off,
    #[clap(name = "TRACE")]
    Trace,
    #[clap(name = "DEBUG")]
    Debug,
    #[clap(name = "INFO")]
    Info,
    #[clap(name = "WARN")]
    Warn,
    #[clap(name = "ERROR")]
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Trace
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Off => write!(f, "Off"),
            LogLevel::Trace => write!(f, "Trace"),
            LogLevel::Debug => write!(f, "Debug"),
            LogLevel::Info => write!(f, "Info"),
            LogLevel::Warn => write!(f, "Warn"),
            LogLevel::Error => write!(f, "Error"),
        }
    }
}

// for string parsing
impl TryFrom<&str> for LogLevel {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, <LogLevel as TryFrom<&str>>::Error> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "off" => Ok(Self::Off),
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err("Invalid log level"),
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(log_level: LogLevel) -> Self {
        match log_level {
            LogLevel::Off => LevelFilter::OFF,
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

#[derive(Debug)]
pub struct Logger {
    current_level: LogLevel,
    level_reload_handle: reload::Handle<LevelFilter, Layered<EnvFilter, Registry>>,
    _guard: WorkerGuard,
}

#[allow(dead_code)]
impl Logger {
    pub fn init<T: AsRef<Path>>(
        level: LogLevel,
        log_dir: T,
        log_file: T,
        rust_log: &str,
    ) -> Result<Self, SetGlobalDefaultError> {
        std::env::set_var("RUST_LOG", rust_log);
        let file_appender = tracing_appender::rolling::never(log_dir, log_file);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let (level_filter, level_filter_handle) = reload::Layer::new(LevelFilter::from(level));

        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(level_filter)
            .with(Layer::new().with_writer(io::stderr))
            .with(Layer::new().with_ansi(false).with_writer(non_blocking));

        tracing::subscriber::set_global_default(subscriber)?;

        Ok(Self {
            _guard: guard,
            current_level: level,
            level_reload_handle: level_filter_handle,
        })
    }

    /// Returns the previous (log) level after setting it to the new one.
    pub fn set_log_level(&mut self, level: LogLevel) -> LogLevel {
        let old = self.current_level;
        self.level_reload_handle
            .modify(|filter| *filter = LevelFilter::from(level))
            .expect("Could not set log level");
        self.current_level = level;
        old
    }

    pub fn current_log_level(&self) -> LogLevel {
        self.current_level
    }
}
