CREATE TABLE access_tokens (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    access_token TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    valid BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_access_tokens_user_id ON access_tokens(user_id);
CREATE INDEX idx_access_tokens_access_token ON access_tokens(access_token);
