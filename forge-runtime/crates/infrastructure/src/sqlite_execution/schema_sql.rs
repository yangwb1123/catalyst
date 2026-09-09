pub(super) const APPLICATION_ID: i64 = 0x4652_4131;

pub(super) const ADMISSIONS: &str = "CREATE TABLE attempt_admissions (
    cursor INTEGER PRIMARY KEY CHECK(cursor BETWEEN 1 AND 1024),
    attempt_id TEXT NOT NULL UNIQUE,
    idempotency_key TEXT NOT NULL UNIQUE,
    request_json TEXT NOT NULL CHECK(length(CAST(request_json AS BLOB)) BETWEEN 1 AND 65536),
    request_sha256 TEXT NOT NULL CHECK(length(request_sha256) = 64)
) STRICT";

pub(super) const EVENTS: &str = "CREATE TABLE attempt_events (
    cursor INTEGER PRIMARY KEY REFERENCES attempt_admissions(cursor),
    event_id TEXT NOT NULL UNIQUE,
    message_id TEXT NOT NULL UNIQUE,
    event_json TEXT NOT NULL CHECK(length(CAST(event_json AS BLOB)) BETWEEN 1 AND 16384),
    event_sha256 TEXT NOT NULL CHECK(length(event_sha256) = 64)
) STRICT";

pub(super) const OUTBOX: &str = "CREATE TABLE attempt_outbox (
    cursor INTEGER PRIMARY KEY REFERENCES attempt_events(cursor),
    event_id TEXT NOT NULL UNIQUE REFERENCES attempt_events(event_id)
) STRICT";

pub(super) const TABLES: [(&str, &str); 3] = [
    ("attempt_admissions", ADMISSIONS),
    ("attempt_events", EVENTS),
    ("attempt_outbox", OUTBOX),
];

pub(super) const INDEXES: [(&str, &str); 5] = [
    (
        "sqlite_autoindex_attempt_admissions_1",
        "attempt_admissions",
    ),
    (
        "sqlite_autoindex_attempt_admissions_2",
        "attempt_admissions",
    ),
    ("sqlite_autoindex_attempt_events_1", "attempt_events"),
    ("sqlite_autoindex_attempt_events_2", "attempt_events"),
    ("sqlite_autoindex_attempt_outbox_1", "attempt_outbox"),
];
