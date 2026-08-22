package contextpackagecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

const (
	utf8CounterID   = "forgeos.token-counter.utf8-bytes/v1"
	utf8CounterHash = "44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf"
)

type byteCounter struct {
	err error
}

func (counter byteCounter) Identity() TokenizerIdentity {
	return TokenizerIdentity{TokenizerID: utf8CounterID, TokenizerSHA256: utf8CounterHash}
}

func (counter byteCounter) Count(value []byte) (uint64, error) {
	if counter.err != nil {
		return 0, counter.err
	}
	return uint64(len(value)), nil
}

type fixtureEnvelope struct {
	ExpectedPackage ContextPackage `json:"expected_package"`
	Request         BuildRequest   `json:"request"`
}

func loadFixture(t *testing.T) fixtureEnvelope {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "context-package-v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture fixtureEnvelope
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	return fixture
}

func validRequest(t *testing.T) *BuildRequest {
	t.Helper()
	fixture := loadFixture(t)
	return &fixture.Request
}

func stringPointer(value string) *string { return &value }

func contentDigest(value string) string {
	digest := sha256.Sum256([]byte(value))
	return hex.EncodeToString(digest[:])
}

var errCounter = errors.New("counter unavailable")
