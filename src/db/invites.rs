use sqlx::postgres::PgPool;
use sqlx::Row;


#[derive(Clone)]
pub struct InviteQueries {
    pool: PgPool,
}

impl InviteQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn add(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO invites (email, code) VALUES ($1, $2);")
            .bind(email)
            .bind(code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_email(&self, code: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT email FROM invites WHERE code = $1 and activated = false and invite_sent = true;")
            .bind(code)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("email").ok())
    }

    pub async fn activate(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        println!("Activating invite code: {} for email: {}", code, email);

        sqlx::query("UPDATE invites SET activated = true, activated_at = $1 WHERE email = $2 and code = $3;")
            .bind(now)
            .bind(email)
            .bind(code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

}
