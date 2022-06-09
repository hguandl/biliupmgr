-- Your SQL goes here
CREATE TABLE events (
    event_id TEXT NOT NULL PRIMARY KEY,
    event_type TEXT NOT NULL,
    room_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    title TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    file_open_time DATETIME NOT NULL,
    file_size INTEGER NOT NULL,
    duration REAL NOT NULL
);
