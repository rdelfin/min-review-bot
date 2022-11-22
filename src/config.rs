use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fmt::{self, Debug, Formatter},
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub users: BTreeSet<String>,
    pub repo: String,
    pub bot_username: String,
    pub github: GithubConfig,
    pub sleep_period: Duration,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubConfig {
    pub private_key_path: PathBuf,
    pub app_id: u64,
}
