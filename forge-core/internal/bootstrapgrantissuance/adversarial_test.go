package bootstrapgrantissuance

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/grantstate"
)

func TestConflictAuthenticatesButReadsNoClockOrKey(t *testing.T) {
	layout := newIssuanceLayout(t)
	writeMode(t, ledgerPath(layout), layout.docs.ledger, 0o600)
	request := rewriteFixtureRequest(t, layout.docs, func(document map[string]any) {
		bindings := document["bindings"].(map[string]any)
		bindings["source_revision"] = "different-revision"
	})
	writeLeaf(t, layout.config.AuthorityRoot, layout.config.RequestPath, request)
	if err := os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	clock := &fixedClock{err: errors.New("must not be called")}
	before, _ := os.Stat(ledgerPath(layout))
	output, err := issueWith(layout.config, realDependencies(clock))
	if output != nil || ErrorCode(err) != CodeIdempotencyConflict {
		t.Fatalf("conflict = %q, %v", output, err)
	}
	after, _ := os.Stat(ledgerPath(layout))
	if clock.calls != 0 || !before.ModTime().Equal(after.ModTime()) {
		t.Fatalf("conflict used clock or wrote ledger: calls=%d", clock.calls)
	}
}

func TestRollbackFailsBeforePrivateKeyRead(t *testing.T) {
	layout := newIssuanceLayout(t)
	writeMode(t, ledgerPath(layout), layout.docs.ledger, 0o600)
	request := rewriteFixtureRequest(t, layout.docs, func(document map[string]any) {
		document["idempotency_key"] = "fixture-request-key-0002"
	})
	writeLeaf(t, layout.config.AuthorityRoot, layout.config.RequestPath, request)
	if err := os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	clock := &fixedClock{value: fixtureStoredAt - 1}
	output, err := issueWith(layout.config, realDependencies(clock))
	if output != nil || ErrorCode(err) != CodeClockRejected || clock.calls != 1 {
		t.Fatalf("rollback = %q, %v, clock calls %d", output, err, clock.calls)
	}
}

func TestExpiredRequestFailsBeforePrivateKeyRead(t *testing.T) {
	layout := newIssuanceLayout(t)
	if err := os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	clock := &fixedClock{value: 1_700_000_301_000}
	output, err := issueWith(layout.config, realDependencies(clock))
	if output != nil || ErrorCode(err) != CodeClockRejected || clock.calls != 1 {
		t.Fatalf("expired = %q, %v, clock calls %d", output, err, clock.calls)
	}
}

func TestWrongPinAndSignatureFailBeforeClockAndKey(t *testing.T) {
	for _, kind := range []string{"pin", "signature"} {
		t.Run(kind, func(t *testing.T) {
			layout := newIssuanceLayout(t)
			if kind == "pin" {
				layout.config.PinnedRootSHA256 = string(bytes.Repeat([]byte{'0'}, 64))
			} else {
				writeLeaf(t, layout.config.AuthorityRoot, layout.config.PolicyPath,
					corruptSignature(t, layout.docs.policy))
			}
			if err := os.Remove(layout.key); err != nil {
				t.Fatal(err)
			}
			clock := &fixedClock{err: errors.New("must not run")}
			_, err := issueWith(layout.config, realDependencies(clock))
			expected := CodeTrustRootRejected
			if kind == "signature" {
				expected = CodePolicyRejected
			}
			if ErrorCode(err) != expected || clock.calls != 0 {
				t.Fatalf("%s = %v, clock calls %d", kind, err, clock.calls)
			}
		})
	}
}

func TestKnownPublicGoldenAuthorityIsRejectedBeforeClockAndKey(t *testing.T) {
	layout := newIssuanceLayout(t)
	raw, err := os.ReadFile(filepath.Join("..", "..", "..", "docs", "contracts",
		"fixtures", "bootstrap-grant-issuance-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture map[string]json.RawMessage
	if err = json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	root := decodeDocument(t, fixture["trust_root"])
	layout.config.PinnedRootSHA256 = root["root_sha256"].(string)
	writeLeaf(t, layout.config.AuthorityRoot, layout.config.TrustRootPath,
		fixture["trust_root"])
	if err = os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	clock := &fixedClock{err: errors.New("must not be called")}
	output, err := issueWith(layout.config, realDependencies(clock))
	if output != nil || ErrorCode(err) != CodeTrustRootRejected || clock.calls != 0 {
		t.Fatalf("known fixture root = %q, %v, clock calls %d", output, err, clock.calls)
	}
}

func TestOversizedSourceRevisionIsRejectedBeforeClockAndKeyForBothDecisions(t *testing.T) {
	for _, disposition := range []string{"allow", "deny"} {
		t.Run(disposition, func(t *testing.T) {
			layout := newIssuanceLayout(t)
			policy := layout.docs.policy
			if disposition == "deny" {
				policy = rewriteSigned(t, policy, "policy_sha256",
					[]byte("forgeos.bootstrap-grant-policy.v1\x00"),
					[]byte("forgeos.bootstrap-grant-policy.signature.v1\x00"),
					layout.docs.policySeed, func(document map[string]any) {
						document["disposition"] = "deny"
					})
				writeLeaf(t, layout.config.AuthorityRoot, layout.config.PolicyPath, policy)
			}
			policySHA := decodeDocument(t, policy)["policy_sha256"].(string)
			request := rewriteFixtureRequest(t, layout.docs, func(document map[string]any) {
				document["policy_sha256"] = policySHA
				document["bindings"].(map[string]any)["source_revision"] = strings.Repeat("x", 161)
			})
			writeLeaf(t, layout.config.AuthorityRoot, layout.config.RequestPath, request)
			if err := os.Remove(layout.key); err != nil {
				t.Fatal(err)
			}
			clock := &fixedClock{err: errors.New("must not be called")}
			output, err := issueWith(layout.config, realDependencies(clock))
			if output != nil || ErrorCode(err) != CodeRequestRejected || clock.calls != 0 {
				t.Fatalf("oversized revision = %q, %v, clock calls %d", output, err, clock.calls)
			}
		})
	}
}

func TestWrongRawSeedFailsWithoutStateWrite(t *testing.T) {
	for _, size := range []int{31, 32, 33} {
		t.Run(fmt.Sprintf("bytes-%d", size), func(t *testing.T) {
			layout := newIssuanceLayout(t)
			writeMode(t, layout.key, make([]byte, size), 0o600)
			clock := &fixedClock{value: fixtureStoredAt}
			output, err := issueWith(layout.config, realDependencies(clock))
			if output != nil || ErrorCode(err) != CodeIssuerKeyRejected || clock.calls != 1 {
				t.Fatalf("wrong seed = %q, %v", output, err)
			}
			if _, err := os.Stat(ledgerPath(layout)); !os.IsNotExist(err) {
				t.Fatalf("wrong key created ledger: %v", err)
			}
		})
	}
}

func TestInvalidLedgerSignatureFailsBeforeClockAndKey(t *testing.T) {
	layout := newIssuanceLayout(t)
	writeMode(t, ledgerPath(layout), corruptSignature(t, layout.docs.ledger), 0o600)
	if err := os.Remove(layout.key); err != nil {
		t.Fatal(err)
	}
	clock := &fixedClock{err: errors.New("must not run")}
	_, err := issueWith(layout.config, realDependencies(clock))
	if ErrorCode(err) != CodeLedgerRejected || clock.calls != 0 {
		t.Fatalf("invalid ledger = %v, clock calls %d", err, clock.calls)
	}
}

func TestUnsafeAuthorityLeafModesAreNotRepaired(t *testing.T) {
	for _, leaf := range []string{"trust", "key"} {
		t.Run(leaf, func(t *testing.T) {
			layout := newIssuanceLayout(t)
			path := layout.key
			expected := CodeIssuerKeyRejected
			if leaf == "trust" {
				path = filepath.Join(layout.config.AuthorityRoot, layout.config.TrustRootPath)
				expected = CodeTrustRootRejected
			}
			if err := os.Chmod(path, 0o644); err != nil {
				t.Fatal(err)
			}
			clock := &fixedClock{value: fixtureStoredAt}
			_, err := issueWith(layout.config, realDependencies(clock))
			if ErrorCode(err) != expected {
				t.Fatalf("unsafe %s mode: %v", leaf, err)
			}
			info, _ := os.Stat(path)
			if info.Mode().Perm() != 0o644 {
				t.Fatalf("unsafe mode was repaired to %04o", info.Mode().Perm())
			}
		})
	}
}

func rewriteFixtureRequest(t *testing.T, docs fixtureDocuments,
	mutate func(map[string]any)) []byte {
	t.Helper()
	return rewriteSigned(t, docs.request, "request_sha256",
		[]byte("forgeos.bootstrap-grant-request.v1\x00"),
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"), docs.requestSeed, mutate)
}

func corruptSignature(t *testing.T, data []byte) []byte {
	t.Helper()
	document := decodeDocument(t, data)
	signature := document["signature"].(map[string]any)
	value := []byte(signature["signature_base64url"].(string))
	if value[0] == 'A' {
		value[0] = 'B'
	} else {
		value[0] = 'A'
	}
	signature["signature_base64url"] = string(value)
	encoded, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

type memoryState struct {
	closeErr    error
	closes      int
	commitErr   error
	commits     int
	corruptRead bool
	leaves      map[string][]byte
	snapshot    grantstate.Snapshot
}

func (s *memoryState) Current() (grantstate.Snapshot, error) {
	value := s.snapshot
	value.Data = clone(value.Data)
	if s.corruptRead && s.commits > 0 {
		value = grantstate.Snapshot{Present: true, Data: []byte("corrupt")}
	}
	return value, nil
}

func (s *memoryState) Commit(expected grantstate.Snapshot, next []byte) error {
	s.commits++
	if s.commitErr != nil {
		return s.commitErr
	}
	s.snapshot = grantstate.Snapshot{Present: true, Data: clone(next)}
	return nil
}

func (s *memoryState) ReadLeaf(path string, max int64, mode fs.FileMode) ([]byte, error) {
	value, found := s.leaves[path]
	if !found || int64(len(value)) > max || mode != 0o600 {
		return nil, os.ErrNotExist
	}
	return clone(value), nil
}

func (s *memoryState) Close() error {
	s.closes++
	return s.closeErr
}
