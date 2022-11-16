use octocrab::{models::pulls::PullRequest, Error};

/// Returns data for a given pull request.
/// # Arguments
/// * `num` - The pull request number ot fetch
pub async fn get_pr(num: u64) -> Result<PullRequest, Error> {
    octocrab::instance()
        .pulls("rdelfin", "async-zmq")
        .media_type(octocrab::params::pulls::MediaType::Full)
        .get(num)
        .await
}
