use serde_json::Value;
use sqlx::postgres::PgPool;


#[derive(Clone)]
pub struct EventQueries {
    pool: PgPool,
}

impl EventQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn store(
        &self, 
        event_id: &str, 
        room_id: &str, 
        event_type: &str, 
        sender: &str,
        event_json: Value,
        recipients: Option<Vec<&str>>,
        relates_to_event_id: Option<&str>,
        in_reply_to: Option<&str>,
        rel_type: Option<&str>,
        message_id: Option<&str>,
    ) 
    -> Result<(), sqlx::Error> {

        sqlx::query("INSERT INTO events (event_id, room_id, type, sender, recipients, relates_to_event_id, in_reply_to, rel_type, message_id, json) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(event_id)
            .bind(room_id)
            .bind(event_type)
            .bind(sender)
            .bind(recipients)
            .bind(relates_to_event_id)
            .bind(in_reply_to)
            .bind(rel_type)
            .bind(message_id)
            .bind(event_json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

}
