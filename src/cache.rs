use sqlx::{
    sqlite::{SqlitePool, SqlitePoolOptions},
    Executor,
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};

pub struct Cache {
    pool: SqlitePool,
}

impl Cache {
    pub async fn new() -> sqlx::Result<Cache> {
        let connector = Cache {
            pool: SqlitePoolOptions::new()
                .max_connections(10)
                .connect("sqlite::memory:")
                .await?,
        };
        connector.initialise_db().await?;
        Ok(connector)
    }

    async fn initialise_db(&self) -> sqlx::Result<()> {
        self.pool
            .execute(
                "CREATE TABLE last_checked_change (\
                    pr_id UNSIGNED INTEGER NOT NULL PRIMARY KEY,\
                    last_updated_unixus INTEGER NOT NULL\
                );",
            )
            .await?;

        Ok(())
    }

    pub async fn get_all_last_updates(&self) -> sqlx::Result<BTreeMap<u64, SystemTime>> {
        let query = sqlx::query!("SELECT pr_id, last_updated_unixus FROM last_checked_change")
            .fetch_all(&self.pool)
            .await?;

        Ok(query
            .into_iter()
            .map(|row| {
                (
                    row.pr_id.try_into().unwrap(),
                    SystemTime::UNIX_EPOCH
                        + Duration::from_secs(row.last_updated_unixus.try_into().unwrap()),
                )
            })
            .collect())
    }

    pub async fn update_pr(&self, pr_id: u64, update_time: SystemTime) -> sqlx::Result<()> {
        let update_time_unix = update_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is always after unix epoch")
            .as_secs() as i64;
        let pr_id = pr_id as i64;

        sqlx::query!(
            "INSERT OR REPLACE INTO last_checked_change (pr_id, last_updated_unixus) VALUES (?, ?)",
            pr_id,
            update_time_unix,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
