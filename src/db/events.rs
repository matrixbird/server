use serde_json::Value;
use sqlx::postgres::PgPool;


pub struct StoreEventRequest<'a>{
    pub event_id: &'a str,
    pub room_id: &'a str,
    pub event_type: &'a str,
    pub sender: &'a str,
    pub recipients: Option<Vec<&'a str>>,
    pub relates_to_event_id: Option<&'a str>,
    pub in_reply_to: Option<&'a str>,
    pub rel_type: Option<&'a str>,
    pub message_id: Option<&'a str>,
    pub json: Value,
}

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
        event: StoreEventRequest<'_>,
    ) 
    -> Result<(), sqlx::Error> {

        sqlx::query("INSERT INTO events (event_id, room_id, type, sender, recipients, relates_to_event_id, in_reply_to, rel_type, message_id, json) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(event.event_id)
            .bind(event.room_id)
            .bind(event.event_type)
            .bind(event.sender)
            .bind(event.recipients)
            .bind(event.relates_to_event_id)
            .bind(event.in_reply_to)
            .bind(event.rel_type)
            .bind(event.message_id)
            .bind(event.json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

}
