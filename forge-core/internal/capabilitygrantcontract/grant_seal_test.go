package capabilitygrantcontract

import "testing"

func TestGrantSigningPreparationAndFinalizationMatchGolden(t *testing.T) {
	grant := cloneNode(fixtureNode(t, loadFixture(t), "grant"))
	wantDigest := grant["grant_sha256"].(string)
	proof, _ := objectValue(grant, "authority_proof")
	wantProof := proof["proof_base64url"].(string)
	grant["grant_id"], grant["grant_sha256"], proof["proof_base64url"] = "", "", ""
	prepared, digest, err := PrepareGrantForSigning(grant)
	if err != nil || digest != wantDigest {
		t.Fatalf("prepare failed: digest=%s err=%v", digest, err)
	}
	if grant["grant_id"] != "" || grant["grant_sha256"] != "" || proof["proof_base64url"] != "" {
		t.Fatal("preparation mutated the caller's candidate")
	}
	final, err := FinalizeSignedGrant(prepared, wantProof)
	if err != nil {
		t.Fatal(err)
	}
	if err = validateGrant(final); err != nil || final["grant_sha256"] != wantDigest {
		t.Fatalf("final Grant is invalid: %v", err)
	}
}

func TestGrantSigningAPIRejectsStaleOrMalformedInputs(t *testing.T) {
	grant := cloneNode(fixtureNode(t, loadFixture(t), "grant"))
	if _, _, err := PrepareGrantForSigning(grant); err == nil {
		t.Fatal("already sealed Grant was accepted as a candidate")
	}
	proof, _ := objectValue(grant, "authority_proof")
	grant["grant_id"], grant["grant_sha256"], proof["proof_base64url"] = "", "", ""
	prepared, _, err := PrepareGrantForSigning(grant)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = FinalizeSignedGrant(prepared, "not base64url!"); err == nil {
		t.Fatal("malformed proof was accepted")
	}
}

func TestGrantSigningAPIRejectsCyclicProgrammaticInputBeforeClone(t *testing.T) {
	cycle := map[string]any{}
	cycle["self"] = cycle
	if _, _, err := PrepareGrantForSigning(cycle); err == nil {
		t.Fatal("cyclic candidate was accepted")
	}
	grant := cloneNode(fixtureNode(t, loadFixture(t), "grant"))
	proof, _ := objectValue(grant, "authority_proof")
	grant["grant_id"], grant["grant_sha256"], proof["proof_base64url"] = "", "", ""
	prepared, _, err := PrepareGrantForSigning(grant)
	if err != nil {
		t.Fatal(err)
	}
	prepared["scope"] = cycle
	if _, err = FinalizeSignedGrant(prepared, "proof"); err == nil {
		t.Fatal("cyclic prepared Grant was accepted")
	}
}
