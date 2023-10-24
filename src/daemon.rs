use clap::Parser;
use codeowners::Owners;
use futures_util::future::join_all;
use jsonwebtoken::EncodingKey;
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
use opentelemetry::sdk::Resource;
use opentelemetry_api::KeyValue;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::time::Instant;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Parser, Debug)]
#[command(term_width = 0)]
struct Args {
    #[arg(long, short)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing()?;
    let args = Args::parse();
    let config: Config = toml::de::from_slice(&tokio::fs::read(args.config).await?)?;
    info!(config = ?config, "starting node");

    if let Some(datadog_socket) = config.datadog_socket.as_ref() {
        if let Err(e) = MetricsReporter::initialise(datadog_socket) {
            warn!(
                error = ?e,
                "There was an error initialising connection to datadog; continuing"
            );
        }
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
            error!(error = ?e, "there was an error");
        }
        MetricsReporter::report_loop_data(loop_start.elapsed(), config.sleep_period);
        tokio::time::sleep_until(next_awake).await;
        next_awake += config.sleep_period;
    }
}

#[instrument(level = "info", skip_all, err)]
async fn inner_update_loop(
    db: &Cache,
    repo_connector: &RepoConnector<GithubSource>,
    config: &Config,
) -> anyhow::Result<()> {
    let (prs, codeowners, updates) = fetch_pr_info(db, repo_connector, config).await?;

    let process_iter = prs
        .into_iter()
        .map(|pr| process_pr(pr, &updates, &codeowners, config, db, repo_connector));

    join_all(process_iter)
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(())
}

#[instrument(level = "info", skip_all, fields(pr_num = pr.number), err)]
async fn process_pr(
    pr: PullRequest,
    updates: &BTreeMap<u64, SystemTime>,
    codeowners: &Owners,
    config: &Config,
    db: &Cache,
    repo_connector: &RepoConnector<GithubSource>,
) -> anyhow::Result<()> {
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
    Ok(())
}

#[instrument(level = "info", skip_all, err)]
async fn fetch_pr_info(
    db: &Cache,
    repo_connector: &RepoConnector<GithubSource>,
    config: &Config,
) -> anyhow::Result<(Vec<PullRequest>, Owners, BTreeMap<u64, SystemTime>)> {
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

    Ok((prs, codeowners, updates))
}

#[instrument(level = "info", skip_all, fields(pr_num = pr.number), ret)]
fn should_update_pr(
    pr: &PullRequest,
    updates: &BTreeMap<u64, SystemTime>,
    config: &Config,
) -> bool {
    if config.banned_prs.contains(&pr.number) {
        info!("pr is banned");
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

#[instrument(level = "info", skip_all, fields(pr_num = pr_id), ret)]
async fn get_pr_conditional<S: RepoSource>(
    pr_id: u64,
    repo_connector: &RepoConnector<S>,
    codeowners: &Owners,
) -> anyhow::Result<(OwnersConditional, BTreeSet<String>)> {
    let changed_files = repo_connector.get_pr_changed_files(pr_id).await?;
    info!(changed_files =? changed_files, "changed files");
    let changed_files_slc: Vec<&str> = changed_files.iter().map(|f| f.as_ref()).collect();
    Ok((
        OwnersConditional::from_codeowners(codeowners, &changed_files_slc[..]).reduce(),
        changed_files,
    ))
}

#[instrument(
    level = "info",
    skip_all,
    fields(pr_num = pr.number),
    err
)]
async fn update_pr(
    config: &Config,
    pr: &PullRequest,
    repo_connector: &RepoConnector<GithubSource>,
    db: &Cache,
    codeowners: &Owners,
    conditional: OwnersConditional,
    changed_files: BTreeSet<String>,
) -> anyhow::Result<()> {
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
            pr_number = pr.id.0,
            comment = ?comment,
            "would have updated comment",
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

fn setup_tracing() -> anyhow::Result<()> {
    // Install a new OpenTelemetry trace pipeline
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(
            opentelemetry::sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "min_review_bot",
            )])),
        )
        .install_simple()?;

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Configure a custom event formatter
    let format = tracing_subscriber::fmt::format()
        .with_line_number(true)
        .with_thread_names(true) // include the name of the current thread
        .with_timer(tracing_subscriber::fmt::time::SystemTime) // use system time
        .compact(); // use the `Compact` formatting style.

    let console_layer = tracing_subscriber::fmt::layer().event_format(format);

    // Create a tracing subscriber with the default level INFO
    // To change the level, set the environment variable RUST_LOG before
    // running the executable: $ RUST_LOG=error ./exec
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(console_layer)
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
