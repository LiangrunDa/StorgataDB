use std::io;
use std::path::Path;
use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::{filter::EnvFilter, fmt::Layer, layer::SubscriberExt};

pub(crate) fn init_logger<T: AsRef<Path>>(
    level: String,
    log_dir: T,
    log_file: T,
    rust_log: &str,
) -> Result<(), SetGlobalDefaultError> {
    let project_name = env!("CARGO_PKG_NAME");
    let underscored_project_name = project_name.replace("-", "_");
    let rust_log = format!("{rust_log},{underscored_project_name}={level}");
    std::env::set_var("RUST_LOG", rust_log);

    let file_appender = tracing_appender::rolling::never(log_dir, log_file);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(Layer::new().with_writer(io::stderr))
        .with(Layer::new().with_ansi(false).with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
