use serde::Deserialize;
use std::{collections::BTreeSet, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub users: BTreeSet<String>,
    pub repo: String,
    pub bot_username: String,
    pub github: GithubConfig,
    pub sleep_period: Duration,
    pub db_path: PathBuf,
    #[serde(default)]
    pub dry_run: bool,
    // A list of PRs that are banned from being checked
    pub banned_prs: BTreeSet<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubConfig {
    pub private_key_path: PathBuf,
    pub app_id: u64,
}
