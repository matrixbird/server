CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    room_id TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL,
    sender TEXT NOT NULL,
    json JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_event_id ON events(event_id);
CREATE INDEX idx_events_room_id ON events(room_id);
CREATE INDEX idx_events_type ON events(type);
CREATE INDEX idx_events_sender ON events(sender);
CREATE INDEX idx_events_created_at ON events(created_at);

CREATE INDEX idx_events_jsonb ON events USING GIN (json);
