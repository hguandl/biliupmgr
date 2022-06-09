CREATE TABLE uploads (
    id INTEGER NOT NULL PRIMARY KEY,
    event_id TEXT NOT NULL,
    avid INTEGER,
    uploaded BOOLEAN DEFAULT 0,
    visable BOOLEAN DEFAULT 0,
    created_at DATETIME NOT NULL,
    finished_at DATETIME,
    archive TEXT
);