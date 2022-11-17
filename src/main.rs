use crate::github::{GithubSource, Repo, RepoConnector};
use clap::Parser;
use jsonwebtoken::EncodingKey;
use octocrab::{models::AppId, Octocrab};
use std::{env, path::PathBuf};

mod codeowners;
mod github;

#[derive(Parser, Debug)] // requires `derive` feature
#[command(term_width = 0)] // Just to make testing across clap features easier
struct Args {
    #[arg(long, short)]
    pr_num: u64,
    #[arg(long, short)]
    repo: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let args = Args::parse();

    let pem_path = env::var("GITHUB_PRIVATE_KEY_PATH")?;
    let pem_data = tokio::fs::read(PathBuf::from(pem_path)).await?;
    let repo = Repo::from_path(args.repo)?;

    octocrab::initialise(Octocrab::builder().app(
        AppId(env::var("GITHUB_APP_ID")?.parse()?),
        EncodingKey::from_rsa_pem(&pem_data)?,
    ))?;

    let repo_connector = RepoConnector::new(GithubSource::new_authorized(repo.user()).await?, repo);
    let changed_files = repo_connector.get_pr_changed_files(args.pr_num).await?;
    let codeowners_data = repo_connector.get_codeowners_content().await?;
    let codeowners = codeowners::CodeOwners::parse(codeowners_data)?;

    println!("PR ownership by file");
    for (file, owners) in changed_files.into_iter().map(|file| {
        let owners = codeowners.owners(&file).unwrap_or(vec![]);
        (file, owners)
    }) {
        println!("{file}");
        println!("\t{}", owners.join(", "));
    }

    Ok(())
}
