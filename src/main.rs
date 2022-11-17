use crate::github::{GithubSource, Repo, RepoConnector};
use clap::Parser;
use jsonwebtoken::EncodingKey;
use octocrab::{models::AppId, Octocrab};
use std::{env, path::PathBuf};

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
    let codeowners = codeowners::from_reader(codeowners_data.as_bytes());

    println!("PR ownership by file");
    let mut all_owners_lines: Vec<String> = vec![];
    for (file, owners) in changed_files.into_iter().map(|file| {
        let owners = codeowners.of(&file);
        (file, owners)
    }) {
        let (owners_str, owners_lines) = match owners {
            Some(owners) => {
                let owners_vec = owners
                    .iter()
                    .map(|owner| format!("{}", owner))
                    .collect::<Vec<_>>();
                (
                    owners_vec.join(", "),
                    owners_vec
                        .into_iter()
                        .map(|owner| format!("  * {}", owner))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
            None => ("".to_string(), "".to_string()),
        };

        println!("{file}");
        println!("\t{}", owners_str);
        all_owners_lines.push(format!("* {file}\n{owners_lines}"));
    }

    let comment = format!(
        "# File owners\nThe owners of the files being reviewed are:\n{}",
        all_owners_lines.join("\n")
    );

    println!("Sending comment to PR {}:\n{comment}", args.pr_num);
    repo_connector
        .add_or_edit_comment(args.pr_num, comment, env::var("BOT_USERNAME")?)
        .await?;

    Ok(())
}
