//go:build linux && !android

package controlstore

import (
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/binary"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const (
	controlSchemaV1CatalogSHA256 = "d306abca185dbdf0601b2cda5ab0cb214c1eb5d001ac759aab543aab207552cc"
	controlSchemaV1FixtureSHA256 = "156f9c54419d770639cf54322eeb0fc3023e6c7e177d03c3cd5e1026742f66b9"
)

func TestFrozenControlSchemaV1CatalogDigest(t *testing.T) {
	value := openTestStore(t)
	if got := controlCatalogSHA256(t, value.store.db); got != controlSchemaV1CatalogSHA256 {
		t.Fatalf("schema v1 catalog SHA-256 = %s", got)
	}
}

func TestFrozenPhysicalControlSchemaV1FixtureReopens(t *testing.T) {
	state := prepareFrozenControlFixture(t)
	root, err := os.OpenRoot(state)
	if err != nil {
		t.Fatal(err)
	}
	store, err := OpenBound(context.Background(), root)
	if err != nil {
		_ = root.Close()
		t.Fatal(err)
	}
	defer func() {
		_ = store.Close()
		_ = root.Close()
	}()
	if got := controlCatalogSHA256(t, store.db); got != controlSchemaV1CatalogSHA256 {
		t.Fatalf("fixture catalog SHA-256 = %s", got)
	}
}

func prepareFrozenControlFixture(t *testing.T) string {
	t.Helper()
	encoded, err := os.ReadFile("testdata/control-v1.db.gz.base64")
	if err != nil {
		t.Fatal(err)
	}
	compressed, err := base64.StdEncoding.DecodeString(strings.Join(strings.Fields(string(encoded)), ""))
	if err != nil {
		t.Fatal(err)
	}
	reader, err := gzip.NewReader(bytes.NewReader(compressed))
	if err != nil {
		t.Fatal(err)
	}
	raw, err := io.ReadAll(io.LimitReader(reader, 1024*1024+1))
	closeErr := reader.Close()
	if err != nil || closeErr != nil || len(raw) > 1024*1024 {
		t.Fatalf("decode frozen control fixture: read=%v close=%v size=%d", err, closeErr, len(raw))
	}
	if fmt.Sprintf("%x", sha256.Sum256(raw)) != controlSchemaV1FixtureSHA256 {
		t.Fatal("frozen control fixture SHA-256 differs")
	}
	return writeFrozenControlFixture(t, raw)
}

func writeFrozenControlFixture(t *testing.T, raw []byte) string {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	state := filepath.Join(parent, "state")
	if err := os.Mkdir(state, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(state, databaseName), raw, 0o600); err != nil {
		t.Fatal(err)
	}
	return state
}

func controlCatalogSHA256(t *testing.T, db *sql.DB) string {
	t.Helper()
	rows, err := db.Query(`SELECT type, name, tbl_name, sql FROM sqlite_schema
WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name, tbl_name, sql`)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rows.Close() }()
	hash := sha256.New()
	for rows.Next() {
		var fields [4]string
		if err := rows.Scan(&fields[0], &fields[1], &fields[2], &fields[3]); err != nil {
			t.Fatal(err)
		}
		writeCatalogFields(hash, fields)
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	return fmt.Sprintf("%x", hash.Sum(nil))
}

type catalogHashWriter interface {
	Write([]byte) (int, error)
}

func writeCatalogFields(writer catalogHashWriter, fields [4]string) {
	for _, field := range fields {
		var size [8]byte
		binary.BigEndian.PutUint64(size[:], uint64(len(field)))
		_, _ = writer.Write(size[:])
		_, _ = writer.Write([]byte(field))
	}
}
