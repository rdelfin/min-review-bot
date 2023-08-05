use clap::Parser;
use codeowners::Owners;
use jsonwebtoken::EncodingKey;
use log::{error, info, warn, LevelFilter};
use min_review_bot::{
    cache::Cache,
    conditional::OwnersConditional,
    config::Config,
    github::{GithubSource, Repo, RepoConnector, RepoSource},
    metrics::MetricsReporter,
};
use octocrab::{
    models::{pulls::PullRequest, AppId},
    Octocrab,
};
use simple_logger::SimpleLogger;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::time::Instant;

#[derive(Parser, Debug)]
#[command(term_width = 0)]
struct Args {
    #[arg(long, short)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        // sqlx prints *every* query. We don't need that
        .with_module_level("sqlx", LevelFilter::Warn)
        .init()?;
    let args = Args::parse();
    let config: Config = toml::de::from_slice(&tokio::fs::read(args.config).await?)?;
    info!("Config: {config:#?}");

    if let Err(e) = MetricsReporter::initialise(&config.datadog_socket) {
        warn!("There was an error initialising connection to datadog: {e}; continuing")
    }

    let pem_data = tokio::fs::read(PathBuf::from(&config.github.private_key_path)).await?;
    let repo = Repo::from_path(&config.repo)?;

    let db = Cache::new(&config).await?;

    octocrab::initialise(Octocrab::builder().app(
        AppId(config.github.app_id),
        EncodingKey::from_rsa_pem(&pem_data)?,
    ))?;
    let repo_connector = RepoConnector::new(GithubSource::new_authorized(repo.user()).await?, repo);

    let mut next_awake = Instant::now() + config.sleep_period;
    loop {
        let loop_start = Instant::now();
        if let Err(e) = inner_update_loop(&db, &repo_connector, &config).await {
            error!("There was an error: {e}");
        }
        MetricsReporter::report_loop_data(loop_start.elapsed(), config.sleep_period);
        tokio::time::sleep_until(next_awake).await;
        next_awake += config.sleep_period;
    }
}

async fn inner_update_loop(
    db: &Cache,
    repo_connector: &RepoConnector<GithubSource>,
    config: &Config,
) -> anyhow::Result<()> {
    let updates = db.get_all_last_updates().await?;
    let prs: Vec<_> = repo_connector
        .get_open_prs()
        .await?
        .into_iter()
        .filter(|pr| {
            if let Some(user) = &pr.user {
                config.users.contains(&user.login)
            } else {
                false
            }
        })
        .collect();
    let codeowners_data = repo_connector.get_codeowners_content().await?;
    let codeowners = codeowners::from_reader(codeowners_data.as_bytes());

    for pr in prs {
        if should_update_pr(&pr, &updates, config) {
            let (conditional, changed_files) =
                get_pr_conditional(pr.number, repo_connector, &codeowners).await?;
            update_pr(
                config,
                &pr,
                repo_connector,
                db,
                &codeowners,
                conditional,
                changed_files,
            )
            .await?;
        }
    }

    Ok(())
}

fn should_update_pr(
    pr: &PullRequest,
    updates: &BTreeMap<u64, SystemTime>,
    config: &Config,
) -> bool {
    if config.banned_prs.contains(&pr.number) {
        false
    } else {
        updates
            .get(&pr.number)
            .map(|cached_update_time| {
                if let Some(updated_at) = pr.updated_at {
                    cached_update_time < &SystemTime::from(updated_at)
                } else {
                    false
                }
            })
            .unwrap_or(true)
    }
}

async fn get_pr_conditional<S: RepoSource>(
    pr_id: u64,
    repo_connector: &RepoConnector<S>,
    codeowners: &Owners,
) -> anyhow::Result<(OwnersConditional, BTreeSet<String>)> {
    let changed_files = repo_connector.get_pr_changed_files(pr_id).await?;
    info!("Changed files: {changed_files:?}");
    let changed_files_slc: Vec<&str> = changed_files.iter().map(|f| f.as_ref()).collect();
    Ok((
        OwnersConditional::from_codeowners(codeowners, &changed_files_slc[..]).reduce(),
        changed_files,
    ))
}

async fn update_pr(
    config: &Config,
    pr: &PullRequest,
    repo_connector: &RepoConnector<GithubSource>,
    db: &Cache,
    codeowners: &Owners,
    conditional: OwnersConditional,
    changed_files: BTreeSet<String>,
) -> anyhow::Result<()> {
    info!("Updating PR {}", pr.number);
    let file_owners = min_review_bot::display_file_owners(
        &codeowners,
        &changed_files.iter().map(|f| f.as_ref()).collect::<Vec<_>>(),
    );
    let comment = format!(
        r#"# File Owners
The minimum set of reviewers required are:
`{conditional}`
<details>
    <summary>Details</summary>
    {file_owners}
</detils>"#
    );

    if config.dry_run {
        info!(
            "Would have updated comment in PR {} with {comment:?}",
            pr.id.0
        );
    } else {
        repo_connector
            .add_or_edit_comment(pr.number, comment, config.bot_username.clone())
            .await?;
    }

    let updated_at_systime = pr
        .updated_at
        .map(|dt| SystemTime::UNIX_EPOCH + Duration::from_secs(dt.timestamp() as u64))
        .unwrap_or(SystemTime::now());
    db.update_pr(pr.id.0, updated_at_systime).await?;

    Ok(())
}
