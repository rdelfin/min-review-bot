use octocrab::models::pulls::PullRequest;
use std::collections::BTreeSet;
use unidiff::{PatchSet, PatchedFile};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Repo {
    user: String,
    repo: String,
}

impl Repo {
    pub fn new(user: String, repo: String) -> Repo {
        Repo { user, repo }
    }

    pub fn from_path(path: String) -> Result<Repo, Error> {
        let segments: Vec<_> = path.split("/").collect();
        if segments.len() != 2 {
            return Err(Error::InvalidPath(path));
        }

        Ok(Repo::new(segments[0].into(), segments[1].into()))
    }

    pub fn user<'a>(&'a self) -> &'a str {
        &self.user
    }
    pub fn repo<'a>(&'a self) -> &'a str {
        &self.repo
    }
}

pub struct RepoConnector<S: RepoSource> {
    source: S,
    repo: Repo,
}

impl<S: RepoSource> RepoConnector<S> {
    pub fn new(source: S, repo: Repo) -> RepoConnector<S> {
        RepoConnector { source, repo }
    }

    pub async fn get_pr_changed_files(&self, num: u64) -> Result<BTreeSet<String>> {
        let diff = self.source.get_pr_diff(num, &self.repo).await?;
        get_files_from_diff(diff)
    }

    pub async fn get_codeowners_content(&self) -> Result<String> {
        self.source
            .get_file_data("docs/CODEOWNERS".into(), &self.repo)
            .await
    }
}

#[async_trait::async_trait]
pub trait RepoSource {
    /// Returns data for a given pull request.
    /// # Arguments
    /// * `num` - The pull request number to fetch
    /// * `repo` - The repository where you can find this PR
    async fn get_pr_info(&self, num: u64, repo: &Repo) -> Result<PullRequest>;
    async fn get_pr_diff(&self, num: u64, repo: &Repo) -> Result<String>;
    async fn get_file_data(&self, path: String, repo: &Repo) -> Result<String>;
}

#[derive(Debug)]
pub struct GithubSource;

#[async_trait::async_trait]
impl RepoSource for GithubSource {
    async fn get_pr_info(&self, num: u64, repo: &Repo) -> Result<PullRequest> {
        Ok(octocrab::instance()
            .pulls(repo.user(), repo.repo())
            .media_type(octocrab::params::pulls::MediaType::Full)
            .get(num)
            .await?)
    }

    async fn get_pr_diff(&self, num: u64, repo: &Repo) -> Result<String> {
        Ok(octocrab::instance()
            .pulls(repo.user(), repo.repo())
            .media_type(octocrab::params::pulls::MediaType::Full)
            .get_diff(num)
            .await?)
    }

    async fn get_file_data(&self, path: String, repo: &Repo) -> Result<String> {
        let content_items = octocrab::instance()
            .repos(repo.user(), repo.repo())
            .get_content()
            .path(path)
            .send()
            .await?;

        if content_items.items.len() != 1 {
            return Err(Error::GotMulticontent);
        }

        content_items.items[0]
            .content
            .clone()
            .ok_or(Error::EmptyContents)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("github path {0} is an invalid github path")]
    InvalidPath(String),
    #[error("requested file contents but got either none or multiple results")]
    GotMulticontent,
    #[error("requested file contents returned empty")]
    EmptyContents,
    #[error("error talking to github api: {0}")]
    OctocrabError(#[from] octocrab::Error),
    #[error("error parsing diff: {0}")]
    DiffParseError(#[from] unidiff::Error),
    #[error("expected prefix in diff file name {0}, found none")]
    InvalidDiffFile(String),
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

fn get_files_from_diff(diff: String) -> Result<BTreeSet<String>> {
    let mut files_changed = BTreeSet::new();

    let mut patch = PatchSet::new();
    patch.parse(diff)?;

    let new = patch.added_files();
    let modified = patch.modified_files();
    let deleted = patch.removed_files();

    // Some vectors we need to only add the source, others only the destination
    let mut add_src = vec![];
    let mut add_dst = vec![];
    add_src.extend(modified.iter());
    add_src.extend(deleted.iter());
    add_dst.extend(modified.iter());
    add_dst.extend(new.iter());

    files_changed.extend(
        add_src
            .into_iter()
            .map(remove_prefix(false))
            .collect::<Result<Vec<_>>>()?
            .into_iter(),
    );
    files_changed.extend(
        add_dst
            .into_iter()
            .map(remove_prefix(true))
            .collect::<Result<Vec<_>>>()?
            .into_iter(),
    );

    Ok(files_changed)
}

fn remove_prefix(is_target: bool) -> Box<dyn Fn(&PatchedFile) -> Result<String>> {
    Box::new(move |f: &PatchedFile| {
        let file = if is_target {
            &f.target_file
        } else {
            &f.source_file
        };
        let prefix = if is_target { "b/" } else { "a/" };
        if file.starts_with(prefix) {
            Ok(file[2..].to_string())
        } else {
            Err(Error::InvalidDiffFile(file.clone()))
        }
    })
}
