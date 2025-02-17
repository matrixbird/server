CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    local_part TEXT NOT NULL UNIQUE,
    email TEXT UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    active BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_users_user_id ON users(user_id);
CREATE INDEX idx_users_local_part ON users(local_part);
CREATE INDEX idx_users_email ON users(email);
