DROP INDEX IF EXISTS idx_users_user_id;
DROP INDEX IF EXISTS idx_users_local_part;
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_status;
DROP TABLE IF EXISTS users;
DROP TYPE IF EXISTS user_status;
