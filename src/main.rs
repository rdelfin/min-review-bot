use crate::github::{GithubSource, Repo, RepoConnector};
use clap::Parser;

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
    let args = Args::parse();
    let repo_connector = RepoConnector::new(GithubSource, Repo::from_path(args.repo)?);
    println!("PR DIFF");
    println!(
        "{:?}",
        repo_connector.get_pr_changed_files(args.pr_num).await?
    );
    println!("CODEOWNERS");
    println!("{:?}", repo_connector.get_codeowners_content().await?);
    Ok(())
}
