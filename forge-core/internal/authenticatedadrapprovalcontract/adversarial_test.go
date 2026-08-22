package authenticatedadrapprovalcontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestBundleRejectsStrictJSONAndWireMutations(t *testing.T) {
	_, canonical := loadGoldenBundle(t)
	cases := map[string][]byte{
		"duplicate key": append([]byte(`{"authorization_ledger":null,`),
			canonical[1:]...),
		"unknown key":         append([]byte(`{"unknown_field":null,`), canonical[1:]...),
		"trailing whitespace": append(append([]byte(nil), canonical...), ' '),
		"physical LF":         append(append([]byte(nil), canonical...), '\n'),
		"invalid UTF-8": replaceFirstBytes(canonical, []byte("ADR-9002"),
			[]byte{'A', 'D', 'R', '-', '9', '0', '0', 0xff}),
		"raw control": replaceFirstBytes(canonical, []byte("ADR-9002"),
			[]byte{'A', 'D', 'R', '-', 1, '0', '0', '2'}),
		"bidi": replaceFirstBytes(canonical, []byte("ADR-9002"),
			[]byte("ADR-\u202e9002")),
		"fractional integer": replaceFirstBytes(canonical,
			[]byte(`"trust_epoch":1`), []byte(`"trust_epoch":1.0`)),
		"bad base64url": mutateNamedString(canonical, "signature_base64url", '*'),
	}
	for name, raw := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCanonicalBundle(raw); err == nil {
				t.Fatal("strict mutation passed")
			}
		})
	}
}

func TestEveryFrozenDigestFamilyRejectsIndependentMutation(t *testing.T) {
	_, canonical := loadGoldenBundle(t)
	fields := []string{"profile_sha256", "root_sha256", "proposal_binding_sha256",
		"body_sha256", "physical_sha256", "self_sha256", "policy_sha256",
		"revocation_sha256", "approval_sha256", "request_sha256",
		"receipt_sha256", "ledger_sha256"}
	for _, field := range fields {
		t.Run(field, func(t *testing.T) {
			mutated := mutateNamedString(canonical, field, '0')
			if bytes.Equal(mutated, canonical) {
				t.Fatalf("field %s was absent", field)
			}
			if _, err := DecodeCanonicalBundle(mutated); err == nil {
				t.Fatal("digest mutation passed")
			}
		})
	}
}

func TestCanonicalInputAndLedgerRejectJSONFormDrift(t *testing.T) {
	bundle, _ := loadGoldenBundle(t)
	root := goldenRoot(t, bundle)
	input := goldenInput(t, bundle, root)
	encodedPolicy, err := boundedCanonicalJSON(input.policy, maxPolicyBytes, "policy")
	if err != nil {
		t.Fatal(err)
	}
	encodedRequest, _ := boundedCanonicalJSON(input.request, maxRequestBytes, "request")
	encodedSnapshot, _ := boundedCanonicalJSON(input.snapshots[0], maxRevocationBytes, "snapshot")
	if _, err = DecodeAuthorizationInput(EncodedAuthorizationInput{
		ProposalDocument: input.proposal, Policy: append(encodedPolicy, ' '),
		RevocationSnapshots: [][]byte{encodedSnapshot}, Request: encodedRequest}, root); err == nil {
		t.Fatal("noncanonical policy passed")
	}
	ledger := goldenLedger(t, bundle, root)
	ledgerRaw, _ := CanonicalLedgerJSON(ledger)
	if _, err = DecodeCanonicalLedger(append(ledgerRaw, '\n'), root); err == nil {
		t.Fatal("ledger physical LF passed")
	}
	tooDeep := []byte(strings.Repeat("[", maxJSONDepth+1) + "null" +
		strings.Repeat("]", maxJSONDepth+1))
	if _, err = parseStrictJSON(tooDeep, len(tooDeep)); err == nil {
		t.Fatal("excessive JSON depth passed")
	}
	if _, err = parseStrictJSON([]byte(`{"value":"oversized"}`), 5); err == nil {
		t.Fatal("byte ceiling passed")
	}
}

func mutateNamedString(raw []byte, field string, replacement byte) []byte {
	result := append([]byte(nil), raw...)
	marker := []byte(`"` + field + `":"`)
	index := bytes.Index(result, marker)
	if index < 0 {
		return result
	}
	index += len(marker)
	if result[index] == replacement {
		replacement = '1'
	}
	result[index] = replacement
	return result
}

func replaceFirstBytes(raw, old, replacement []byte) []byte {
	index := bytes.Index(raw, old)
	if index < 0 {
		return append([]byte(nil), raw...)
	}
	result := make([]byte, 0, len(raw)-len(old)+len(replacement))
	result = append(result, raw[:index]...)
	result = append(result, replacement...)
	return append(result, raw[index+len(old):]...)
}
