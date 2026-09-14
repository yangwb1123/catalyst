use rusqlite::{Connection, ErrorCode, params};

use super::super::open_database;
use super::fixtures::exact_v29_fixture;

#[test]
fn conversation_change_schema_enforces_closed_values_and_unique_keys() {
    let (_root, database) = exact_v29_fixture();
    let connection = open_database(&database).expect("migrate constraint fixture to v30");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign key checks");
    for invalid in invalid_change_rows() {
        reject_invalid_change(&connection, &invalid);
    }
    connection
        .execute(
            "INSERT INTO conversations VALUES(
               'conversation-new-v30','global',NULL,'New','conversation-new-v30-key',3,3
             )",
            [],
        )
        .expect("add a post-migration Conversation without a legacy baseline");
    insert_valid_creation_change(&connection);
    reject_duplicate_aggregate_version(&connection);
    reject_duplicate_event_entity(&connection);
}

struct InvalidChange {
    label: &'static str,
    cursor: i64,
    conversation_id: &'static str,
    entity_id: &'static str,
    aggregate_version: i64,
    schema_version: i64,
    kind: &'static str,
    time: i64,
}

fn invalid_change_rows() -> [InvalidChange; 7] {
    [
        InvalidChange::valid("cursor").with_cursor(0),
        InvalidChange::valid("conversation").with_conversation("missing-conversation"),
        InvalidChange::valid("entity").with_entity(""),
        InvalidChange::valid("aggregate").with_aggregate_version(0),
        InvalidChange::valid("event schema").with_schema_version(2),
        InvalidChange::valid("kind").with_kind("unsupported"),
        InvalidChange::valid("timestamp").with_time(-1),
    ]
}

impl InvalidChange {
    fn valid(label: &'static str) -> Self {
        Self {
            label,
            cursor: 1,
            conversation_id: "conversation-v30",
            entity_id: "entity",
            aggregate_version: 1,
            schema_version: 1,
            kind: "conversation_created",
            time: 1,
        }
    }

    fn with_cursor(mut self, cursor: i64) -> Self {
        self.cursor = cursor;
        self
    }

    fn with_conversation(mut self, conversation_id: &'static str) -> Self {
        self.conversation_id = conversation_id;
        self
    }

    fn with_entity(mut self, entity_id: &'static str) -> Self {
        self.entity_id = entity_id;
        self
    }

    fn with_aggregate_version(mut self, aggregate_version: i64) -> Self {
        self.aggregate_version = aggregate_version;
        self
    }

    fn with_schema_version(mut self, schema_version: i64) -> Self {
        self.schema_version = schema_version;
        self
    }

    fn with_kind(mut self, kind: &'static str) -> Self {
        self.kind = kind;
        self
    }

    fn with_time(mut self, time: i64) -> Self {
        self.time = time;
        self
    }
}

fn reject_invalid_change(connection: &Connection, change: &InvalidChange) {
    let error = connection
        .execute(
            "INSERT INTO conversation_changes(
               cursor,conversation_id,entity_id,aggregate_version,
               event_schema_version,event_kind,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                change.cursor,
                change.conversation_id,
                change.entity_id,
                change.aggregate_version,
                change.schema_version,
                change.kind,
                change.time
            ],
        )
        .expect_err("invalid conversation change must fail");
    assert!(is_constraint_error(&error), "{}: {error:?}", change.label);
}

fn insert_valid_creation_change(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO conversation_changes(
               cursor,conversation_id,entity_id,aggregate_version,
               event_schema_version,event_kind,created_at_ms
             ) VALUES(1,'conversation-new-v30','conversation-new-v30',1,1,'conversation_created',3)",
            [],
        )
        .expect("insert valid creation change");
}

fn reject_duplicate_aggregate_version(connection: &Connection) {
    let duplicate = connection
        .execute(
            "INSERT INTO conversation_changes(
               cursor,conversation_id,entity_id,aggregate_version,
               event_schema_version,event_kind,created_at_ms
             ) VALUES(2,'conversation-new-v30','prompt-v30',1,1,'prompt_appended',4)",
            [],
        )
        .expect_err("one journal version per Conversation");
    assert!(is_constraint_error(&duplicate), "{duplicate:?}");
}

fn reject_duplicate_event_entity(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO conversation_changes(
               cursor,conversation_id,entity_id,aggregate_version,
               event_schema_version,event_kind,created_at_ms
             ) VALUES(2,'conversation-new-v30','prompt-v30',2,1,'prompt_appended',4)",
            [],
        )
        .expect("insert first Prompt change");
    let duplicate = connection
        .execute(
            "INSERT INTO conversation_changes(
               cursor,conversation_id,entity_id,aggregate_version,
               event_schema_version,event_kind,created_at_ms
             ) VALUES(3,'conversation-new-v30','prompt-v30',3,1,'prompt_appended',5)",
            [],
        )
        .expect_err("one event per canonical Prompt");
    assert!(is_constraint_error(&duplicate), "{duplicate:?}");
}

fn is_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(problem, _)
            if problem.code == ErrorCode::ConstraintViolation
    )
}
