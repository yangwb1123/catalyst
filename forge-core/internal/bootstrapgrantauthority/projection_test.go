package bootstrapgrantauthority

import "testing"

func TestIssuedGrantProjectionComesOnlyFromExactIssuedRecord(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	ledger, err := DecodeLedger(mustCanonical(t, context.document["ledger"]), context.trust)
	if err != nil {
		t.Fatal(err)
	}
	receipt := context.document["receipt"].(map[string]any)
	projection, found, err := LookupIssuedGrant(ledger, receipt["grant_id"].(string),
		receipt["grant_sha256"].(string), receipt["grant_envelope_sha256"].(string),
		receipt["receipt_sha256"].(string), 1)
	if err != nil || !found {
		t.Fatalf("exact issued Grant lookup failed: found=%v err=%v", found, err)
	}
	if len(projection.document["bindings"].(map[string]any)) != 3 ||
		len(projection.document["resources"].([]any)) != 2 ||
		projection.document["grant_policy_sha256"] != context.policy.document["policy_sha256"] ||
		projection.document["grant_request_sha256"] != context.request.document["request_sha256"] {
		t.Fatal("issued Grant execution projection drifted")
	}
	if _, found, err = LookupIssuedGrant(ledger, receipt["grant_id"].(string),
		receipt["grant_sha256"].(string), receipt["grant_envelope_sha256"].(string),
		string(make([]byte, 64)), 1); err != nil || found {
		t.Fatalf("mismatched issuance receipt was resolved: found=%v err=%v", found, err)
	}
}
