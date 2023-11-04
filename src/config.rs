use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub users: HashSet<String>,
    pub repo: String,
    pub bot_username: String,
    pub github: GithubConfig,
    pub sleep_period: Duration,
    pub db_path: PathBuf,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub send_open_telemetry: bool,
    // A list of PRs that are banned from being checked
    pub banned_prs: HashSet<u64>,
    #[serde(default)]
    pub datadog_socket: Option<PathBuf>,
    #[serde(default)]
    pub exclude_owners: HashSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubConfig {
    pub private_key_path: PathBuf,
    pub app_id: u64,
}
