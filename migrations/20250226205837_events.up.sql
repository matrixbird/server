CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    room_id TEXT NOT NULL,
    type TEXT NOT NULL,
    sender TEXT NOT NULL,
    recipients TEXT[],
    relates_to_event_id TEXT,
    in_reply_to TEXT,
    rel_type TEXT,
    message_id TEXT,
    json JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_event_id ON events(event_id);
CREATE INDEX idx_events_room_id ON events(room_id);
CREATE INDEX idx_events_type ON events(type);
CREATE INDEX idx_events_sender ON events(sender);
CREATE INDEX idx_events_recipients ON events(recipients);
CREATE INDEX idx_events_relates_to_event_id ON events(relates_to_event_id);
CREATE INDEX idx_events_in_reply_to ON events(in_reply_to);
CREATE INDEX idx_events_rel_type ON events(rel_type);
CREATE INDEX idx_events_message_id ON events(message_id);
CREATE INDEX idx_events_created_at ON events(created_at);

CREATE INDEX idx_events_jsonb ON events USING GIN (json);
