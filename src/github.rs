/// This file provides a basic interface into Github that can be easily replaced and mocked out for
/// use when testing other parts of the codebase.
use octocrab::{models::pulls::PullRequest, Octocrab};
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
pub struct GithubSource {
    octo_instance: Octocrab,
}

impl GithubSource {
    pub async fn new_authorized(user: &str) -> Result<GithubSource> {
        let mut installation_id = None;
        let mut page = 1u32; // yes, it's 1-indexed
        let mut found = false;
        let mut empty = false;
        while !found && !empty {
            empty = true;
            for installation in octocrab::instance()
                .apps()
                .installations()
                .per_page(100)
                .page(page)
                .send()
                .await?
            {
                empty = false;
                if installation.account.login == user {
                    installation_id = Some(installation.id);
                    found = true;
                    break;
                }
            }
            page += 1;
        }

        let installation_id =
            installation_id.ok_or_else(|| Error::NoInstallationId(user.into()))?;
        let octo_instance = octocrab::instance().installation(installation_id);

        Ok(GithubSource { octo_instance })
    }
}

#[async_trait::async_trait]
impl RepoSource for GithubSource {
    async fn get_pr_info(&self, num: u64, repo: &Repo) -> Result<PullRequest> {
        Ok(self
            .octo_instance
            .pulls(repo.user(), repo.repo())
            .media_type(octocrab::params::pulls::MediaType::Full)
            .get(num)
            .await?)
    }

    async fn get_pr_diff(&self, num: u64, repo: &Repo) -> Result<String> {
        Ok(self
            .octo_instance
            .pulls(repo.user(), repo.repo())
            .media_type(octocrab::params::pulls::MediaType::Full)
            .get_diff(num)
            .await?)
    }

    async fn get_file_data(&self, path: String, repo: &Repo) -> Result<String> {
        let content_items = self
            .octo_instance
            .repos(repo.user(), repo.repo())
            .get_content()
            .path(path)
            .send()
            .await?
            .take_items();

        if content_items.len() != 1 {
            return Err(Error::GotMulticontent);
        }

        let raw_contents = content_items[0]
            .content
            .clone()
            .ok_or(Error::EmptyContents)?
            .replace("\n", "");

        Ok(String::from_utf8(base64::decode(raw_contents)?)?)
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
    #[error("could not find installation ID matching user {0}")]
    NoInstallationId(String),
    #[error("could not decode file as base64: {0}")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("could not decode file as utf8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
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
