use sqlx::postgres::PgPool;
use sqlx::Row;


#[derive(Clone)]
pub struct UserQueries {
    pool: PgPool,
}

impl UserQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn exists(&self, user_id: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE user_id = $1 and status != 'deleted)")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }

    pub async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email)
            .fetch_one(&self.pool)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }

    pub async fn local_part_exists(&self, local_part: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE local_part = $1 and status != 'deleted)")
            .bind(local_part)
            .fetch_one(&self.pool)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }


    pub async fn create(&self, user_id: &str, local_part: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO users (user_id, local_part) VALUES ($1, $2);")
            .bind(user_id)
            .bind(local_part)
            .execute(&self.pool)
            .await?;

        Ok(())
    }


    pub async fn add_email(&self, user_id: &str, email: &str) -> Result<(), anyhow::Error> {

        sqlx::query("UPDATE users SET email = $1 WHERE user_id = $2;")
            .bind(email)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_id_from_email(&self, email: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT user_id FROM users WHERE email = $1;")
            .bind(email)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("user_id").ok())
    }

    pub async fn get_email_from_user_id(&self, user_id: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT email FROM users WHERE user_id = $1;")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("email").ok())
    }
}
