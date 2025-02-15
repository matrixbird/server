CREATE TABLE invites (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    code TEXT NOT NULL UNIQUE,
    activated BOOLEAN DEFAULT FALSE,
    invited_by TEXT NOT NULL DEFAULT 'matrixbird',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    invite_sent BOOLEAN DEFAULT FALSE,
    invite_sent_at TIMESTAMP WITH TIME ZONE,
    activated_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_invites_email ON invites(email);
CREATE INDEX idx_invites_code ON invites(code);
