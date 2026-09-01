package controlstore

import (
	"context"
	"database/sql"
	"fmt"
	"sort"
)

const applicationID = 0x464f5247

type schemaObject struct {
	kind, name, table, ddl string
}

var schemaObjects = []schemaObject{
	tableObject("control_schema", `CREATE TABLE control_schema (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    schema_identity TEXT NOT NULL CHECK (schema_identity = 'forgeos.control-store/v1'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1)
) STRICT`),
	tableObject("aggregate_heads", `CREATE TABLE aggregate_heads (
    aggregate_type TEXT NOT NULL CHECK (length(aggregate_type) BETWEEN 1 AND 32),
    aggregate_id TEXT NOT NULL CHECK (length(aggregate_id) = 30),
    current_version INTEGER NOT NULL CHECK (current_version >= 1),
    PRIMARY KEY (aggregate_type, aggregate_id)
) STRICT, WITHOUT ROWID`),
	tableObject("control_events", `CREATE TABLE control_events (
    global_sequence INTEGER PRIMARY KEY CHECK (global_sequence >= 1),
    command_id TEXT NOT NULL CHECK (length(command_id) = 30),
    event_id TEXT NOT NULL CHECK (length(event_id) = 30),
    message_id TEXT NOT NULL CHECK (length(message_id) = 30),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 30),
    causation_id TEXT CHECK (causation_id IS NULL OR length(causation_id) = 30),
    source_component TEXT NOT NULL CHECK (length(source_component) BETWEEN 1 AND 32),
    source_sequence INTEGER NOT NULL CHECK (source_sequence >= 1),
    aggregate_type TEXT NOT NULL CHECK (length(aggregate_type) BETWEEN 1 AND 32),
    aggregate_id TEXT NOT NULL CHECK (length(aggregate_id) = 30),
    aggregate_version INTEGER NOT NULL CHECK (aggregate_version >= 1),
    occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms BETWEEN 0 AND 253402300799999),
    event_sha256 TEXT NOT NULL CHECK (length(event_sha256) = 64),
    event_bytes BLOB NOT NULL CHECK (length(event_bytes) BETWEEN 1 AND 262144),
    FOREIGN KEY (command_id) REFERENCES command_receipts (command_id) DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (causation_id) REFERENCES message_index (message_id) DEFERRABLE INITIALLY DEFERRED
) STRICT`),
	indexObject("control_events_command_id_ix", "control_events",
		`CREATE INDEX control_events_command_id_ix ON control_events (command_id)`),
	indexObject("control_events_event_id_uq", "control_events",
		`CREATE UNIQUE INDEX control_events_event_id_uq ON control_events (event_id)`),
	indexObject("control_events_message_id_uq", "control_events",
		`CREATE UNIQUE INDEX control_events_message_id_uq ON control_events (message_id)`),
	indexObject("control_events_source_sequence_uq", "control_events",
		`CREATE UNIQUE INDEX control_events_source_sequence_uq ON control_events (source_component, source_sequence)`),
	indexObject("control_events_aggregate_version_uq", "control_events",
		`CREATE UNIQUE INDEX control_events_aggregate_version_uq ON control_events (aggregate_type, aggregate_id, aggregate_version)`),
	tableObject("command_receipts", `CREATE TABLE command_receipts (
    idempotency_key TEXT PRIMARY KEY CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    command_id TEXT NOT NULL CHECK (length(command_id) = 30),
    message_id TEXT NOT NULL CHECK (length(message_id) = 30),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 30),
    causation_id TEXT CHECK (causation_id IS NULL OR length(causation_id) = 30),
    request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
    request_bytes BLOB NOT NULL CHECK (length(request_bytes) BETWEEN 1 AND 262144),
    aggregate_type TEXT NOT NULL CHECK (length(aggregate_type) BETWEEN 1 AND 32),
    aggregate_id TEXT NOT NULL CHECK (length(aggregate_id) = 30),
    expected_version INTEGER NOT NULL CHECK (expected_version >= 0),
    aggregate_version INTEGER NOT NULL CHECK (aggregate_version > expected_version),
    first_global_sequence INTEGER NOT NULL CHECK (first_global_sequence >= 1),
    last_global_sequence INTEGER NOT NULL CHECK (last_global_sequence >= first_global_sequence),
    result_sha256 TEXT NOT NULL CHECK (length(result_sha256) = 64),
    result_bytes BLOB NOT NULL CHECK (length(result_bytes) <= 262144),
    issued_at_unix_ms INTEGER NOT NULL CHECK (issued_at_unix_ms BETWEEN 0 AND 253402300799999),
    committed_at_unix_ms INTEGER NOT NULL CHECK (committed_at_unix_ms BETWEEN 0 AND 253402300799999),
    FOREIGN KEY (causation_id) REFERENCES message_index (message_id) DEFERRABLE INITIALLY DEFERRED
) STRICT, WITHOUT ROWID`),
	indexObject("command_receipts_command_id_uq", "command_receipts",
		`CREATE UNIQUE INDEX command_receipts_command_id_uq ON command_receipts (command_id)`),
	indexObject("command_receipts_message_id_uq", "command_receipts",
		`CREATE UNIQUE INDEX command_receipts_message_id_uq ON command_receipts (message_id)`),
	tableObject("message_index", `CREATE TABLE message_index (
    message_id TEXT PRIMARY KEY CHECK (length(message_id) = 30),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 30),
    emitted_at_unix_ms INTEGER NOT NULL CHECK (emitted_at_unix_ms BETWEEN 0 AND 253402300799999),
    journal_kind TEXT NOT NULL CHECK (journal_kind IN ('command', 'control_event', 'inbox_event')),
    journal_sequence INTEGER NOT NULL CHECK (journal_sequence >= 1),
    UNIQUE (journal_kind, journal_sequence)
) STRICT, WITHOUT ROWID`),
	tableObject("outbox_messages", `CREATE TABLE outbox_messages (
    outbox_sequence INTEGER PRIMARY KEY CHECK (outbox_sequence >= 1),
    message_id TEXT NOT NULL CHECK (length(message_id) = 30),
    source_event_id TEXT NOT NULL CHECK (length(source_event_id) = 30),
    destination TEXT NOT NULL CHECK (length(destination) BETWEEN 1 AND 64),
    message_sha256 TEXT NOT NULL CHECK (length(message_sha256) = 64),
    message_bytes BLOB NOT NULL CHECK (length(message_bytes) BETWEEN 1 AND 262144),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms BETWEEN 0 AND 253402300799999),
    FOREIGN KEY (source_event_id) REFERENCES control_events (event_id)
) STRICT`),
	indexObject("outbox_messages_message_id_uq", "outbox_messages",
		`CREATE UNIQUE INDEX outbox_messages_message_id_uq ON outbox_messages (message_id)`),
	tableObject("outbox_acknowledgements", `CREATE TABLE outbox_acknowledgements (
    message_id TEXT PRIMARY KEY CHECK (length(message_id) = 30),
    delivered_at_unix_ms INTEGER NOT NULL CHECK (delivered_at_unix_ms BETWEEN 0 AND 253402300799999),
    FOREIGN KEY (message_id) REFERENCES outbox_messages (message_id)
) STRICT, WITHOUT ROWID`),
	tableObject("inbox_sources", `CREATE TABLE inbox_sources (
    stream_id TEXT PRIMARY KEY CHECK (length(stream_id) BETWEEN 1 AND 128),
    source_component TEXT NOT NULL CHECK (length(source_component) BETWEEN 1 AND 32),
    current_sequence INTEGER NOT NULL CHECK (current_sequence >= 1)
) STRICT, WITHOUT ROWID`),
	tableObject("inbox_messages", `CREATE TABLE inbox_messages (
    inbox_sequence INTEGER PRIMARY KEY CHECK (inbox_sequence >= 1),
    stream_id TEXT NOT NULL CHECK (length(stream_id) BETWEEN 1 AND 128),
    source_component TEXT NOT NULL CHECK (length(source_component) BETWEEN 1 AND 32),
    source_sequence INTEGER NOT NULL CHECK (source_sequence >= 1),
    message_id TEXT NOT NULL CHECK (length(message_id) = 30),
    event_id TEXT NOT NULL CHECK (length(event_id) = 30),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 30),
    causation_id TEXT CHECK (causation_id IS NULL OR length(causation_id) = 30),
    occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms BETWEEN 0 AND 253402300799999),
    event_sha256 TEXT NOT NULL CHECK (length(event_sha256) = 64),
    event_bytes BLOB NOT NULL CHECK (length(event_bytes) BETWEEN 1 AND 262144),
    received_at_unix_ms INTEGER NOT NULL CHECK (received_at_unix_ms BETWEEN 0 AND 253402300799999),
    FOREIGN KEY (stream_id) REFERENCES inbox_sources (stream_id),
    FOREIGN KEY (causation_id) REFERENCES message_index (message_id) DEFERRABLE INITIALLY DEFERRED
) STRICT`),
	indexObject("inbox_messages_stream_sequence_uq", "inbox_messages",
		`CREATE UNIQUE INDEX inbox_messages_stream_sequence_uq ON inbox_messages (stream_id, source_sequence)`),
	indexObject("inbox_messages_stream_message_uq", "inbox_messages",
		`CREATE UNIQUE INDEX inbox_messages_stream_message_uq ON inbox_messages (stream_id, message_id)`),
}

var immutableTables = []string{
	"control_schema", "control_events", "command_receipts", "message_index", "outbox_messages",
	"outbox_acknowledgements", "inbox_messages",
}

func init() {
	for _, table := range immutableTables {
		schemaObjects = append(schemaObjects, immutableTrigger(table, "update"), immutableTrigger(table, "delete"))
	}
}

func tableObject(name, ddl string) schemaObject {
	return schemaObject{kind: "table", name: name, table: name, ddl: ddl}
}

func indexObject(name, table, ddl string) schemaObject {
	return schemaObject{kind: "index", name: name, table: table, ddl: ddl}
}

func immutableTrigger(table, operation string) schemaObject {
	name := table + "_no_" + operation
	ddl := fmt.Sprintf("CREATE TRIGGER %s BEFORE %s ON %s BEGIN SELECT RAISE(ABORT, '%s is append-only'); END",
		name, operation, table, table)
	return schemaObject{kind: "trigger", name: name, table: table, ddl: ddl}
}

func migrateOrValidate(ctx context.Context, db *sql.DB) error {
	version, identity, count, err := schemaHeader(ctx, db)
	if err != nil {
		return err
	}
	if count == 0 && version == 0 && identity == 0 {
		if err := initializeSchema(ctx, db); err != nil {
			return err
		}
	} else if version != schemaVersion || identity != applicationID {
		return fmt.Errorf("%w: application_id=%d user_version=%d", ErrSchemaIncompatible, identity, version)
	}
	return validateSchema(ctx, db)
}

func validateExistingSchema(ctx context.Context, db *sql.DB) error {
	version, identity, count, err := schemaHeader(ctx, db)
	if err != nil {
		return err
	}
	var mode string
	if err := db.QueryRowContext(ctx, `PRAGMA journal_mode`).Scan(&mode); err != nil {
		return fmt.Errorf("%w: read journal_mode", ErrSchemaIncompatible)
	}
	if count == 0 && version == 0 && identity == 0 {
		if mode != "delete" && mode != "wal" {
			return fmt.Errorf("%w: pristine journal_mode=%q", ErrSchemaIncompatible, mode)
		}
		return validatePristineDatabase(ctx, db)
	}
	if count == 0 || version != schemaVersion || identity != applicationID {
		return fmt.Errorf("%w: application_id=%d user_version=%d", ErrSchemaIncompatible, identity, version)
	}
	if mode != "wal" {
		return fmt.Errorf("%w: journal_mode=%q", ErrSchemaIncompatible, mode)
	}
	if err := validateSchemaObjects(ctx, db); err != nil {
		return err
	}
	if err := validateSchemaIdentity(ctx, db); err != nil {
		return err
	}
	return validateRelationalState(ctx, db)
}

func validatePristineDatabase(ctx context.Context, db *sql.DB) error {
	var objects int
	if err := db.QueryRowContext(ctx, `SELECT count(*) FROM sqlite_schema`).Scan(&objects); err != nil {
		return err
	}
	pages, err := readPragmaInt(ctx, db, "PRAGMA page_count")
	if err != nil {
		return err
	}
	freelist, err := readPragmaInt(ctx, db, "PRAGMA freelist_count")
	if err != nil {
		return err
	}
	if objects != 0 || pages != 1 || freelist != 0 {
		return fmt.Errorf("%w: non-pristine zero-identity database", ErrSchemaIncompatible)
	}
	return validateSQLiteIntegrity(ctx, db)
}

func schemaHeader(ctx context.Context, db *sql.DB) (int, int, int, error) {
	version, err := readPragmaInt(ctx, db, "PRAGMA user_version")
	if err != nil {
		return 0, 0, 0, err
	}
	identity, err := readPragmaInt(ctx, db, "PRAGMA application_id")
	if err != nil {
		return 0, 0, 0, err
	}
	var count int
	err = db.QueryRowContext(ctx, `SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'`).Scan(&count)
	return version, identity, count, err
}

func readPragmaInt(ctx context.Context, db *sql.DB, query string) (int, error) {
	var value int
	if err := db.QueryRowContext(ctx, query).Scan(&value); err != nil {
		return 0, fmt.Errorf("read schema pragma: %w", err)
	}
	return value, nil
}

func initializeSchema(ctx context.Context, db *sql.DB) error {
	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin control schema migration: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	for _, object := range schemaObjects {
		if _, err := tx.ExecContext(ctx, object.ddl); err != nil {
			return fmt.Errorf("create control schema object %s: %w", object.name, err)
		}
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO control_schema VALUES (1, 'forgeos.control-store/v1', 1)`); err != nil {
		return fmt.Errorf("write control schema identity: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `PRAGMA application_id = 1179603527`); err != nil {
		return fmt.Errorf("write control application identity: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `PRAGMA user_version = 1`); err != nil {
		return fmt.Errorf("write control schema version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit control schema migration: %w", err)
	}
	return nil
}

func validateSchema(ctx context.Context, db *sql.DB) error {
	if err := validatePragmas(ctx, db); err != nil {
		return err
	}
	if err := validateSchemaObjects(ctx, db); err != nil {
		return err
	}
	if err := validateSchemaIdentity(ctx, db); err != nil {
		return err
	}
	return validateRelationalState(ctx, db)
}

func validatePragmas(ctx context.Context, db *sql.DB) error {
	checks := []struct {
		query string
		want  int
	}{
		{"PRAGMA application_id", applicationID}, {"PRAGMA user_version", schemaVersion},
		{"PRAGMA foreign_keys", 1}, {"PRAGMA trusted_schema", 0},
		{"PRAGMA recursive_triggers", 0}, {"PRAGMA synchronous", 2},
		{"PRAGMA busy_timeout", 5000},
	}
	for _, check := range checks {
		got, err := readPragmaInt(ctx, db, check.query)
		if err != nil || got != check.want {
			return fmt.Errorf("%w: %s=%d", ErrSchemaIncompatible, check.query, got)
		}
	}
	var mode string
	if err := db.QueryRowContext(ctx, `PRAGMA journal_mode`).Scan(&mode); err != nil || mode != "wal" {
		return fmt.Errorf("%w: journal_mode=%q", ErrSchemaIncompatible, mode)
	}
	return nil
}

func validateSchemaObjects(ctx context.Context, db *sql.DB) error {
	rows, err := db.QueryContext(ctx, `SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name`)
	if err != nil {
		return fmt.Errorf("read control schema catalog: %w", err)
	}
	defer func() { _ = rows.Close() }()
	actual := make(map[string]schemaObject, len(schemaObjects))
	for rows.Next() {
		var object schemaObject
		if err := rows.Scan(&object.kind, &object.name, &object.table, &object.ddl); err != nil {
			return fmt.Errorf("read control schema object: %w", err)
		}
		actual[object.kind+":"+object.name] = object
	}
	if err := rows.Err(); err != nil {
		return err
	}
	if len(actual) != len(schemaObjects) {
		return fmt.Errorf("%w: schema object count=%d want=%d", ErrSchemaIncompatible, len(actual), len(schemaObjects))
	}
	for _, want := range schemaObjects {
		got, ok := actual[want.kind+":"+want.name]
		if !ok || got != want {
			return fmt.Errorf("%w: schema object %s differs", ErrSchemaIncompatible, want.name)
		}
	}
	return nil
}

func validateSchemaIdentity(ctx context.Context, db *sql.DB) error {
	var singleton, version int
	var identity string
	err := db.QueryRowContext(ctx, `SELECT singleton, schema_identity, schema_version FROM control_schema`).
		Scan(&singleton, &identity, &version)
	if err != nil || singleton != 1 || identity != "forgeos.control-store/v1" || version != schemaVersion {
		return fmt.Errorf("%w: control schema identity differs", ErrSchemaIncompatible)
	}
	return nil
}

const commandCoverageCheck = `SELECT count(*) FROM command_receipts r LEFT JOIN (SELECT command_id, count(*) AS total, min(global_sequence) AS first_sequence, max(global_sequence) AS last_sequence, min(aggregate_type) AS minimum_type, max(aggregate_type) AS maximum_type, min(aggregate_id) AS minimum_id, max(aggregate_id) AS maximum_id, min(aggregate_version) AS first_version, max(aggregate_version) AS last_version FROM control_events GROUP BY command_id) e USING (command_id) WHERE e.total IS NULL OR e.first_sequence <> r.first_global_sequence OR e.last_sequence <> r.last_global_sequence OR e.last_sequence - e.first_sequence + 1 <> e.total OR e.total <> r.aggregate_version - r.expected_version OR e.minimum_type <> r.aggregate_type OR e.maximum_type <> r.aggregate_type OR e.minimum_id <> r.aggregate_id OR e.maximum_id <> r.aggregate_id OR e.first_version <> r.expected_version + 1 OR e.last_version <> r.aggregate_version`

const controlSourceOrderCheck = `SELECT count(*) FROM (SELECT source_sequence, row_number() OVER (PARTITION BY source_component ORDER BY global_sequence) AS expected FROM control_events) WHERE source_sequence <> expected`

const aggregateVersionOrderCheck = `SELECT count(*) FROM (SELECT aggregate_version, row_number() OVER (PARTITION BY aggregate_type, aggregate_id ORDER BY global_sequence) AS expected FROM control_events) WHERE aggregate_version <> expected`

const inboxSourceOrderCheck = `SELECT count(*) FROM (SELECT source_sequence, row_number() OVER (PARTITION BY stream_id ORDER BY inbox_sequence) AS expected FROM inbox_messages) WHERE source_sequence <> expected`

const commandCausationCheck = `SELECT count(*) FROM command_receipts e LEFT JOIN message_index c ON c.message_id = e.causation_id WHERE e.causation_id IS NOT NULL AND (c.message_id IS NULL OR c.message_id = e.message_id OR c.correlation_id <> e.correlation_id OR c.emitted_at_unix_ms > e.issued_at_unix_ms OR (c.journal_kind IN ('command', 'control_event') AND c.journal_sequence >= e.first_global_sequence))`

const controlEventCausationCheck = `SELECT count(*) FROM control_events e LEFT JOIN message_index c ON c.message_id = e.causation_id LEFT JOIN command_receipts command_cause ON command_cause.command_id = e.command_id AND command_cause.message_id = e.causation_id LEFT JOIN control_events event_cause ON event_cause.command_id = e.command_id AND event_cause.message_id = e.causation_id AND event_cause.global_sequence < e.global_sequence WHERE e.causation_id IS NULL OR c.message_id IS NULL OR c.correlation_id <> e.correlation_id OR c.emitted_at_unix_ms > e.occurred_at_unix_ms OR (command_cause.message_id IS NULL AND event_cause.message_id IS NULL)`

const inboxCausationCheck = `SELECT count(*) FROM inbox_messages e LEFT JOIN message_index c ON c.message_id = e.causation_id WHERE e.causation_id IS NOT NULL AND (c.message_id IS NULL OR c.message_id = e.message_id OR c.correlation_id <> e.correlation_id OR c.emitted_at_unix_ms > e.occurred_at_unix_ms OR (c.journal_kind = 'inbox_event' AND c.journal_sequence >= e.inbox_sequence))`

func validateRelationalState(ctx context.Context, db *sql.DB) error {
	checks := []string{
		`SELECT CASE WHEN count(*) = 0 THEN 0 WHEN min(global_sequence) = 1 AND max(global_sequence) = count(*) THEN 0 ELSE 1 END FROM control_events`,
		`SELECT count(*) FROM (SELECT source_component FROM control_events GROUP BY source_component HAVING min(source_sequence) <> 1 OR max(source_sequence) <> count(*))`,
		controlSourceOrderCheck,
		`SELECT CASE WHEN count(*) = 0 THEN 0 WHEN min(outbox_sequence) = 1 AND max(outbox_sequence) = count(*) THEN 0 ELSE 1 END FROM outbox_messages`,
		`SELECT CASE WHEN count(*) = 0 THEN 0 WHEN min(inbox_sequence) = 1 AND max(inbox_sequence) = count(*) THEN 0 ELSE 1 END FROM inbox_messages`,
		`SELECT count(*) FROM (SELECT aggregate_type, aggregate_id FROM control_events GROUP BY aggregate_type, aggregate_id HAVING min(aggregate_version) <> 1 OR max(aggregate_version) <> count(*))`,
		aggregateVersionOrderCheck,
		`SELECT count(*) FROM aggregate_heads h LEFT JOIN (SELECT aggregate_type, aggregate_id, max(aggregate_version) AS version FROM control_events GROUP BY aggregate_type, aggregate_id) e USING (aggregate_type, aggregate_id) WHERE e.version IS NULL OR e.version <> h.current_version`,
		`SELECT count(*) FROM control_events e LEFT JOIN aggregate_heads h USING (aggregate_type, aggregate_id) WHERE h.aggregate_id IS NULL`,
		`SELECT count(*) FROM inbox_sources s LEFT JOIN (SELECT stream_id, min(source_sequence) AS minimum, max(source_sequence) AS maximum, count(*) AS total FROM inbox_messages GROUP BY stream_id) m USING (stream_id) WHERE m.maximum IS NULL OR m.minimum <> 1 OR m.maximum <> m.total OR m.maximum <> s.current_sequence`,
		`SELECT count(*) FROM inbox_messages m LEFT JOIN inbox_sources s USING (stream_id) WHERE s.stream_id IS NULL`,
		`SELECT count(*) FROM inbox_messages m JOIN inbox_sources s USING (stream_id) WHERE m.source_component <> s.source_component`,
		inboxSourceOrderCheck,
		commandCoverageCheck,
		`SELECT CASE WHEN (SELECT count(*) FROM message_index) = (SELECT count(*) FROM command_receipts) + (SELECT count(*) FROM control_events) + (SELECT count(*) FROM inbox_messages) THEN 0 ELSE 1 END`,
		`SELECT count(*) FROM message_index JOIN outbox_messages USING (message_id)`,
		`SELECT count(*) FROM command_receipts r LEFT JOIN message_index m ON m.message_id = r.message_id AND m.journal_kind = 'command' WHERE m.message_id IS NULL OR m.correlation_id <> r.correlation_id OR m.emitted_at_unix_ms <> r.issued_at_unix_ms OR m.journal_sequence <> r.first_global_sequence`,
		`SELECT count(*) FROM control_events e LEFT JOIN message_index m ON m.message_id = e.message_id AND m.journal_kind = 'control_event' WHERE m.message_id IS NULL OR m.correlation_id <> e.correlation_id OR m.emitted_at_unix_ms <> e.occurred_at_unix_ms OR m.journal_sequence <> e.global_sequence`,
		`SELECT count(*) FROM inbox_messages e LEFT JOIN message_index m ON m.message_id = e.message_id AND m.journal_kind = 'inbox_event' WHERE m.message_id IS NULL OR m.correlation_id <> e.correlation_id OR m.emitted_at_unix_ms <> e.occurred_at_unix_ms OR m.journal_sequence <> e.inbox_sequence`,
		commandCausationCheck,
		controlEventCausationCheck,
		inboxCausationCheck,
	}
	for _, query := range checks {
		var failures int
		if err := db.QueryRowContext(ctx, query).Scan(&failures); err != nil || failures != 0 {
			return fmt.Errorf("%w: relational consistency check failed", ErrCorruptStore)
		}
	}
	return validateSQLiteIntegrity(ctx, db)
}

func validateSQLiteIntegrity(ctx context.Context, db *sql.DB) error {
	var result string
	if err := db.QueryRowContext(ctx, `PRAGMA quick_check(1)`).Scan(&result); err != nil || result != "ok" {
		return fmt.Errorf("%w: SQLite quick_check=%q", ErrCorruptStore, result)
	}
	rows, err := db.QueryContext(ctx, `PRAGMA foreign_key_check`)
	if err != nil {
		return fmt.Errorf("%w: foreign key check failed", ErrCorruptStore)
	}
	defer func() { _ = rows.Close() }()
	if rows.Next() {
		return fmt.Errorf("%w: foreign key violation", ErrCorruptStore)
	}
	return rows.Err()
}

func sortedSchemaObjectNames() []string {
	names := make([]string, 0, len(schemaObjects))
	for _, object := range schemaObjects {
		names = append(names, object.name)
	}
	sort.Strings(names)
	return names
}
