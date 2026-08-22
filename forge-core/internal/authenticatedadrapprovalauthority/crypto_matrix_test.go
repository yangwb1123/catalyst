//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestEveryExternalInputSignatureCategoryFailsIndependently(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*testing.T, *serviceFixture)
	}{
		{"policy", func(_ *testing.T, fixture *serviceFixture) {
			mutateNodeSignature(fixture.policy["signature"].(map[string]any),
				"signature_base64url")
		}},
		{"revocation", func(_ *testing.T, fixture *serviceFixture) {
			mutateNodeSignature(fixture.revocation["signature"].(map[string]any),
				"signature_base64url")
		}},
		{"approval authority", func(t *testing.T, fixture *serviceFixture) {
			record := fixture.request["approval_records"].([]any)[0].(map[string]any)
			mutateNodeSignature(record["authority_proof"].(map[string]any),
				"proof_base64url")
			fixture.sealRequest(t)
		}},
		{"approval separation of duty", func(t *testing.T, fixture *serviceFixture) {
			record := fixture.request["approval_records"].([]any)[0].(map[string]any)
			mutateNodeSignature(record["separation_of_duty"].(map[string]any),
				"proof_base64url")
			fixture.sealRequest(t)
		}},
		{"request", func(_ *testing.T, fixture *serviceFixture) {
			mutateNodeSignature(fixture.request["signature"].(map[string]any),
				"signature_base64url")
		}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			fixture := newServiceFixture(t)
			test.mutate(t, fixture)
			config := fixture.config(t)
			result, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
			if result != nil {
				t.Fatal("bad external signature returned output")
			}
			assertCode(t, err, codeSignatureRejected)
			assertStateLeavesAbsent(t, config)
		})
	}
}

func TestReceiptAndLedgerSignatureCategoriesFailIndependently(t *testing.T) {
	for _, category := range []string{"receipt", "ledger"} {
		t.Run(category, func(t *testing.T) {
			fixture := newServiceFixture(t)
			result, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t),
				fixture.trust())
			if err != nil {
				t.Fatal(err)
			}
			bundle, err := decodeTestObject(result.Verified().CanonicalJSON())
			if err != nil {
				t.Fatal(err)
			}
			ledger := bundle["authorization_ledger"].(map[string]any)
			if category == "ledger" {
				mutateNodeSignature(ledger["signature"].(map[string]any),
					"signature_base64url")
			} else {
				mutateReceiptCopies(bundle, ledger)
				sealTestLedger(t, fixture, ledger)
			}
			verified, err := VerifyBundle(testCanonical(t, bundle), fixture.trust())
			if verified != nil {
				t.Fatal("bad stored signature returned a verified view")
			}
			assertCode(t, err, codeSignatureRejected)
		})
	}
}

func TestMismatchedStateSeedProducesNoLedger(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	testWritePrivate(t, filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath),
		bytes.Repeat([]byte{9}, 32))
	result, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if result != nil {
		t.Fatal("mismatched state seed returned output")
	}
	assertCode(t, err, codeSignerKeyRejected)
	assertLedgerAbsent(t, config)
}

func mutateReceiptCopies(bundle, ledger map[string]any) {
	receipt := bundle["authorization_receipt"].(map[string]any)
	mutateNodeSignature(receipt["signature"].(map[string]any), "signature_base64url")
	entries := ledger["entries"].([]any)
	entries[len(entries)-1].(map[string]any)["receipt"] = cloneTestObject(receipt)
	bundle["authorization_result"].(map[string]any)["receipt"] = cloneTestObject(receipt)
}

func mutateNodeSignature(node map[string]any, field string) {
	value := node[field].(string)
	replacement := byte('A')
	if value[0] == replacement {
		replacement = 'B'
	}
	node[field] = string(replacement) + value[1:]
}

func assertLedgerAbsent(t *testing.T, config Config) {
	t.Helper()
	_, err := os.Lstat(filepath.Join(config.AuthorityRoot, config.StateDir, stateLedgerFile))
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("failed authorization created a ledger: %v", err)
	}
}
