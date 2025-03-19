use sqlx::postgres::PgPool;
use sqlx::Row;


#[derive(Clone)]
pub struct AccessTokenQueries {
    pool: PgPool,
}

impl AccessTokenQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn add(&self, user_id: &str, access_token: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO access_tokens (user_id, access_token) VALUES ($1, $2);")
            .bind(user_id)
            .bind(access_token)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get(&self, user_id: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT access_token FROM access_tokens WHERE user_id = $1 and valid = true ORDER by created_at DESC LIMIT 1;")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("access_token").ok())
    }

    pub async fn invalidate(&self, user_id: &str) -> Result<(), anyhow::Error> {

        sqlx::query("UPDATE access_tokens SET valid = false WHERE user_id = $1;")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

}
