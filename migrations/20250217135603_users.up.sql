CREATE TYPE user_status AS ENUM (
    'limited',
    'active', 
    'suspended',
    'banned',
    'deactivated',
    'deleted'
);

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE, -- Matrix ID
    local_part TEXT NOT NULL UNIQUE, 
    password TEXT,
    email TEXT UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    status user_status NOT NULL DEFAULT 'limited',
    status_changed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    status_reason TEXT,
    status_changed_by INTEGER REFERENCES users(id),

    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_user_id ON users(user_id);
CREATE INDEX idx_users_local_part ON users(local_part);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(status);
