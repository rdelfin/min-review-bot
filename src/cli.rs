use clap::Parser;
use jsonwebtoken::EncodingKey;
use min_review_bot::{
    conditional::OwnersConditional,
    github::{GithubSource, Repo, RepoConnector},
};
use octocrab::{Octocrab, models::AppId};
use std::{env, path::PathBuf};

#[derive(Parser, Debug)] // requires `derive` feature
#[command(term_width = 0)] // Just to make testing across clap features easier
struct Args {
    #[arg(long, short)]
    pr_num: u64,
    #[arg(long, short)]
    repo: String,
    #[arg(long, short)]
    exclude_owners: Vec<String>,
    #[arg(long, short)]
    update_github: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let args = Args::parse();
    let exclude_owners = args.exclude_owners.into_iter().collect();

    let pem_path = env::var("GITHUB_PRIVATE_KEY_PATH")?;
    let pem_data = tokio::fs::read(PathBuf::from(pem_path)).await?;
    let repo = Repo::from_path(&args.repo)?;

    octocrab::initialise(
        Octocrab::builder()
            .app(
                AppId(env::var("GITHUB_APP_ID")?.parse()?),
                EncodingKey::from_rsa_pem(&pem_data)?,
            )
            .build()?,
    );

    let repo_connector = RepoConnector::new(GithubSource::new_authorized(repo.user()).await?, repo);
    let changed_files = repo_connector.get_pr_changed_files(args.pr_num).await?;
    let codeowners_data = repo_connector.get_codeowners_content().await?;
    let codeowners = codeowners::from_reader(codeowners_data.as_bytes());

    let changed_files_slc: Vec<&str> = changed_files.iter().map(|f| f.as_ref()).collect();
    let conditional = OwnersConditional::from_codeowners(&codeowners, &changed_files_slc[..])
        .remove_all(&exclude_owners)
        .unwrap_or(OwnersConditional::And(Vec::new()))
        .reduce();

    let file_owners = min_review_bot::display_file_owners(&codeowners, &changed_files_slc[..]);
    println!("Required reviewers: {conditional}");

    let comment = format!(
        r#"# File Owners
The minimum set of reviewers required are:
`{conditional}`
<details>
    <summary>Details</summary>
    {file_owners}
</details>"#
    );

    if args.update_github {
        println!("Updating comment on PR {}/{}:", args.repo, args.pr_num);
        println!("{comment}");
        repo_connector
            .add_or_edit_comment(args.pr_num, comment, env::var("BOT_USERNAME")?)
            .await?;
    } else {
        println!(
            "Would have updated coment on PR {}/{} to:",
            args.repo, args.pr_num
        );
        println!("{comment}");
    }

    Ok(())
}
