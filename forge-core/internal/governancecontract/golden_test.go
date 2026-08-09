package governancecontract

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type goldenFixture struct {
	Records []goldenEntry `json:"records"`
}

type goldenEntry struct {
	DigestDomain string          `json:"digest_domain"`
	Expected     goldenExpected  `json:"expected"`
	Record       json.RawMessage `json:"record"`
}

type goldenExpected struct {
	CanonicalPayloadJSON string `json:"canonical_payload_json"`
	CanonicalRecordJSON  string `json:"canonical_record_json"`
	CanonicalSHA256      string `json:"canonical_sha256"`
}

func TestGoldenFixture(t *testing.T) {
	fixture := loadGoldenFixture(t)
	records := make([]*Record, 0, len(fixture.Records))
	encodedSet := bytes.NewBufferString("[")
	for index, entry := range fixture.Records {
		if index > 0 {
			encodedSet.WriteByte(',')
		}
		record := assertGoldenEntry(t, entry)
		records = append(records, record)
		encodedSet.WriteString(entry.Expected.CanonicalRecordJSON)
	}
	encodedSet.WriteByte(']')
	if err := ValidateRecordSet(records); err != nil {
		t.Fatalf("ValidateRecordSet: %v", err)
	}
	if _, err := DecodeRecordSet(encodedSet.Bytes()); err != nil {
		t.Fatalf("DecodeRecordSet: %v", err)
	}
}

func assertGoldenEntry(t *testing.T, entry goldenEntry) *Record {
	t.Helper()
	record, err := DecodeRecord([]byte(entry.Expected.CanonicalRecordJSON))
	if err != nil {
		t.Fatalf("DecodeRecord(%s): %v", entry.DigestDomain, err)
	}
	if got := string(record.PayloadJSON()); got != entry.Expected.CanonicalPayloadJSON {
		t.Fatalf("payload JSON mismatch\ngot:  %s\nwant: %s", got, entry.Expected.CanonicalPayloadJSON)
	}
	if got := string(record.RecordJSON()); got != entry.Expected.CanonicalRecordJSON {
		t.Fatalf("record JSON mismatch\ngot:  %s\nwant: %s", got, entry.Expected.CanonicalRecordJSON)
	}
	if record.Digest() != entry.Expected.CanonicalSHA256 {
		t.Fatalf("digest = %s, want %s", record.Digest(), entry.Expected.CanonicalSHA256)
	}
	if digestDomain(record.Kind()) != entry.DigestDomain {
		t.Fatalf("digest domain = %s, want %s", digestDomain(record.Kind()), entry.DigestDomain)
	}
	assertRawRecordCanonical(t, entry)
	return record
}

func assertRawRecordCanonical(t *testing.T, entry goldenEntry) {
	t.Helper()
	node, err := parseStrictJSON(entry.Record)
	if err != nil {
		t.Fatalf("parse fixture record: %v", err)
	}
	canonical, err := canonicalJSON(node)
	if err != nil {
		t.Fatalf("canonical fixture record: %v", err)
	}
	if string(canonical) != entry.Expected.CanonicalRecordJSON {
		t.Fatalf("fixture record does not canonicalize to expected bytes")
	}
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "governance-evidence-claim-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	return fixture
}
