CREATE TABLE emails (
    id SERIAL PRIMARY KEY,
    message_id TEXT NOT NULL UNIQUE,
    envelope_from TEXT NOT NULL,
    envelope_to TEXT NOT NULL,
    email_json JSONB NOT NULL,
    processed BOOLEAN DEFAULT FALSE,
    processed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_emails_message_id ON emails(message_id);
CREATE INDEX idx_emails_envelope_from ON emails(envelope_from);
CREATE INDEX idx_emails_envelope_to ON emails(envelope_to);
CREATE INDEX idx_emails_created_at ON emails(created_at);

CREATE INDEX idx_emails_jsonb ON emails USING GIN (email_json);
