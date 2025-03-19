use serde_json::Value;
use sqlx::postgres::PgPool;


#[derive(Debug, Clone)]
#[derive(sqlx::FromRow)]
pub struct UnprocessedEmail { 
    pub message_id: String, 
    pub envelope_to: String,
    pub email_json: Value
}


#[derive(Clone)]
pub struct EmailQueries {
    pool: PgPool,
}


impl EmailQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn store(
        &self, 
        message_id: &str, 
        envelope_from: &str, 
        envelope_to: &str,
        email_json: Value,
    ) 
    -> Result<(), sqlx::Error> {

        sqlx::query("INSERT INTO emails (message_id, envelope_from, envelope_to, email_json) VALUES ($1, $2, $3, $4)")
            .bind(message_id)
            .bind(envelope_from)
            .bind(envelope_to)
            .bind(email_json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_processed(&self, message_id: &str, event_id: String) -> Result<(), anyhow::Error> {

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        sqlx::query("UPDATE emails SET processed = true, processed_at = $1, event_id = $2 WHERE message_id = $3;")
            .bind(now)
            .bind(event_id)
            .bind(message_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_unprocessed(&self) -> Result<Vec<UnprocessedEmail>, anyhow::Error> {

        let emails = sqlx::query_as::<_, UnprocessedEmail>("SELECT message_id, envelope_to, email_json FROM emails WHERE processed = false ORDER BY created_at ASC;")
            .fetch_all(&self.pool)
            .await?;

        Ok(emails)
    }
}
