use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Raft: Ip:port of all kv servers
    /// at least one peer address is required
    /// usage:
    /// ./kv-rs --peer-addr 127.0.0.1:8080 --peer-addr 127.0.0.1:8081
    /// ./kv-rs --peer-addr 127.0.0.1:8080 127.0.0.0.1:8081
    #[arg(short = 'p', long, env, num_args = 1.., value_delimiter = ' ')]
    peer_addr: Vec<String>,

    /// Raft: Ip address of the server.
    #[arg(short = 'a', long, env)]
    self_addr: String,

    /// Ip address of the kv server.
    #[arg(short = 'k', long, env, default_value = "0.0.0.0:6379")]
    kv_addr: String,

    /// Relative path to the server's data directory.
    #[arg(short = 'd', long, env, default_value = "./data/kv_server/storage")]
    directory: PathBuf,

    /// Relative path to the server's raft state file.
    #[arg(short = 'r', long, env, default_value = "./data/raft/raft_state")]
    raft_state_file: PathBuf,

    /// Set the log level.
    #[arg(long = "ll", long, env, default_value = "debug")]
    log_level: String,

    /// Logging filter
    #[arg(long, env, default_value = "tokio=error,tarpc=error,raft_lite=info")]
    rust_log: String,
}

impl Args {
    pub fn log_level(&self) -> String {
        self.log_level.clone()
    }

    pub fn data_dir(&self) -> &Path {
        self.directory.as_path()
    }

    pub fn self_addr(&self) -> String {
        self.self_addr.clone()
    }

    pub fn peer_addr(&self) -> Vec<String> {
        self.peer_addr.clone()
    }

    pub fn rust_log(&self) -> &str {
        &self.rust_log
    }

    pub fn raft_state_file(&self) -> PathBuf {
        self.raft_state_file.clone()
    }

    pub fn kv_addr(&self) -> String {
        self.kv_addr.clone()
    }
}

pub fn parse_args() -> Args {
    Args::parse()
}
