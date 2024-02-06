/// This module is copied from https://github.com/robatipoor/rustfulapi
use std::io;
use tracing::{subscriber, Subscriber};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};
use tracing_subscriber::fmt::Layer;

fn create_subscriber<W>(
    name: &str,
    env_filter: EnvFilter,
    writer: W,
) -> impl Subscriber + Sync + Send
    where
        W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    Registry::default()
        .with(env_filter)
        .with(Layer::new().with_writer(io::stdout))
        // .with(Layer::new().with_writer(writer))
        // .with(JsonStorageLayer)
        // .with(BunyanFormattingLayer::new(name.into(), std::io::stdout))
        .with(BunyanFormattingLayer::new(name.into(), writer))
}

pub fn init_subscriber<S>(subscriber: S) -> anyhow::Result<()>
    where
        S: Subscriber + Send + Sync + 'static,
{
    LogTracer::init()?;
    subscriber::set_global_default(subscriber)?;
    Ok(())
}

pub fn init(
    level: String,
    rust_log: &str
) -> anyhow::Result<WorkerGuard> {
    let project_name = env!("CARGO_PKG_NAME");
    let underscored_project_name = project_name.replace("-", "_");
    let rust_log = format!("{rust_log},{underscored_project_name}={level}");
    std::env::set_var("RUST_LOG", rust_log);

    let file_appender = RollingFileAppender::new(Rotation::DAILY, "./data/logs", "kv.log");
    let (file_appender, file_appender_guard) = tracing_appender::non_blocking(file_appender);
    init_subscriber(create_subscriber(
        "kv",
        EnvFilter::from_default_env(),
        file_appender,
    ))?;
    Ok(file_appender_guard)
}
