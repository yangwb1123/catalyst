//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"reflect"
	"testing"
)

func TestStoredAuthorizationFreshAndReplayProjectSamePrerequisiteSource(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	fresh, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	freshSource, err := fresh.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	replay, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	replaySource, err := replay.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(freshSource, replaySource) {
		t.Fatal("fresh and exact replay prerequisite sources differ")
	}
	if bytes.Equal(fresh.Verified().CanonicalJSON(), replay.Verified().CanonicalJSON()) {
		t.Fatal("fresh and replay delivery dispositions unexpectedly match")
	}
	assertPrerequisiteSource(t, fixture, freshSource)
	for _, raw := range [][]byte{freshSource.SignatureProfileJSON,
		freshSource.ApprovalTrustRootJSON, freshSource.ProposalDocument,
		freshSource.ProposalBindingJSON, freshSource.AuthorizationReceiptJSON,
		freshSource.AuthorizationLedgerSignatureJSON} {
		if bytes.Contains(raw, []byte("delivery_disposition")) {
			t.Fatal("unsigned delivery disposition leaked into prerequisite source")
		}
	}
}

func TestStoredAuthorizationAccessorsDeepCopyCallerOwnedBytes(t *testing.T) {
	fixture := newServiceFixture(t)
	stored, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	want, err := stored.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	mutated, err := stored.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	mutable := [][]byte{mutated.SignatureProfileJSON, mutated.ApprovalTrustRootJSON,
		mutated.ProposalDocument, mutated.ProposalBindingJSON,
		mutated.AuthorizationReceiptJSON, mutated.AuthorizationLedgerSignatureJSON}
	for _, raw := range mutable {
		if len(raw) == 0 {
			t.Fatal("prerequisite source contains an empty exact artifact")
		}
		raw[0] ^= 1
	}
	verified := stored.Verified()
	canonical := verified.CanonicalJSON()
	canonical[0] ^= 1
	verified.canonical[0] ^= 1
	again, err := stored.AcceptancePrerequisite()
	if err != nil || !reflect.DeepEqual(again, want) {
		t.Fatalf("caller mutation reached stored prerequisite: %v", err)
	}
	againVerified := stored.Verified()
	if againVerified == nil || !bytes.Equal(againVerified.CanonicalJSON(),
		stored.verified.CanonicalJSON()) {
		t.Fatal("caller mutation reached stored verified view")
	}
}

func TestVerifiedBundleAndUnsignedDispositionCannotForgeStoredAuthorization(t *testing.T) {
	fixture := newServiceFixture(t)
	stored, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	bundle, err := decodeTestObject(stored.Verified().CanonicalJSON())
	if err != nil {
		t.Fatal(err)
	}
	bundle["authorization_result"].(map[string]any)["delivery_disposition"] = "exact_replay"
	verified, err := VerifyBundle(testCanonical(t, bundle), fixture.trust())
	if err != nil || verified == nil {
		t.Fatalf("unsigned disposition-only bundle did not verify: %v", err)
	}
	if _, present := reflect.TypeOf(verified).MethodByName("AcceptancePrerequisite"); present {
		t.Fatal("generic VerifiedBundle exposes a persistence projection")
	}
	want, err := stored.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	dispositionOnly := *stored
	dispositionOnly.verified = *cloneVerifiedBundle(verified)
	if got, projectionErr := dispositionOnly.AcceptancePrerequisite(); projectionErr == nil || !reflect.DeepEqual(got, AcceptancePrerequisiteSource{}) {
		t.Fatal("internal bundle mutation preserved a stored capability")
	}
	again, err := stored.AcceptancePrerequisite()
	if err != nil || !reflect.DeepEqual(again, want) {
		t.Fatalf("generic disposition mutation affected stored projection: %v", err)
	}
	forged := &StoredAuthorization{verified: *cloneVerifiedBundle(verified),
		ledgerCanonical: cloneBytes(stored.ledgerCanonical), trust: fixture.trust()}
	if source, projectionErr := forged.AcceptancePrerequisite(); projectionErr == nil ||
		!reflect.DeepEqual(source, AcceptancePrerequisiteSource{}) {
		t.Fatal("unsealed generic verification forged a prerequisite source")
	}
}

func TestStoredAuthorizationRejectsDeniedZeroForgedAndNoncurrentProjection(t *testing.T) {
	var nilStored *StoredAuthorization
	for name, stored := range map[string]*StoredAuthorization{
		"nil": nilStored, "zero": {},
	} {
		t.Run(name, func(t *testing.T) {
			if source, err := stored.AcceptancePrerequisite(); err == nil ||
				!reflect.DeepEqual(source, AcceptancePrerequisiteSource{}) {
				t.Fatal("invalid capability projected a prerequisite")
			}
		})
	}
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	stored, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if err != nil {
		t.Fatal(err)
	}
	forged := *stored
	forged.ledgerCanonical = cloneBytes(forged.ledgerCanonical)
	forged.ledgerCanonical[0] ^= 1
	if _, err = forged.AcceptancePrerequisite(); ErrorCode(err) != codeStateRejected {
		t.Fatalf("forged capability error = %v", err)
	}
	stale := fixture.trust()
	stale.ObservedAtUnixMS = stored.Verified().AuthorizationExpiresAtUnixMS()
	if sealed, sealErr := newStoredAuthorization(stored.Verified(),
		stored.ledgerCanonical, stale); sealErr == nil || sealed != nil {
		t.Fatal("noncurrent generic verification became a stored authorization")
	}

	deniedFixture := newServiceFixture(t)
	denyFixturePolicy(t, deniedFixture)
	denied, err := AuthorizeAndStore(deniedFixture.config(t), deniedFixture.encoded(t),
		deniedFixture.trust())
	if err != nil || denied == nil || denied.Verified() == nil {
		t.Fatalf("denied audit outcome was not stored: %v", err)
	}
	if source, projectionErr := denied.AcceptancePrerequisite(); ErrorCode(projectionErr) != codeAuthorizationNotCurrent ||
		!reflect.DeepEqual(source, AcceptancePrerequisiteSource{}) {
		t.Fatal("denied stored outcome projected an acceptance prerequisite")
	}
}

func assertPrerequisiteSource(t *testing.T, fixture *serviceFixture,
	source AcceptancePrerequisiteSource) {
	t.Helper()
	if !bytes.Equal(source.SignatureProfileJSON,
		testCanonical(t, testLoadObject(t, "../../../docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json")["signature_profile"])) ||
		!bytes.Equal(source.ApprovalTrustRootJSON, testCanonical(t, fixture.root)) ||
		!bytes.Equal(source.ProposalDocument, fixture.proposal) ||
		!bytes.Equal(source.ProposalBindingJSON,
			testCanonical(t, fixture.policy["proposal_binding"])) {
		t.Fatal("prerequisite exact source artifacts differ")
	}
	var receipt struct {
		ReceiptSHA256 string `json:"receipt_sha256"`
	}
	if err := json.Unmarshal(source.AuthorizationReceiptJSON, &receipt); err != nil ||
		receipt.ReceiptSHA256 != source.AuthorizationReceiptSHA256 {
		t.Fatalf("receipt semantic digest differs: %v", err)
	}
	physical := sha256.Sum256(source.AuthorizationReceiptJSON)
	if source.AuthorizationReceiptPhysicalSHA256 != hex.EncodeToString(physical[:]) ||
		source.ApprovalTrustRootSHA256 != fixture.trust().PinnedTrustRootSHA256 ||
		source.ApprovalTrustEpoch != fixture.trust().PinnedTrustEpoch ||
		source.ObservedAtUnixMS != fixture.trust().ObservedAtUnixMS ||
		source.RevocationHighWaterSequence != fixture.trust().RevocationHighWaterSequence ||
		source.RevocationHighWaterSHA256 != fixture.trust().RevocationHighWaterSHA256 ||
		source.AuthorizationLedgerLastSequence != 1 ||
		source.AuthorizationLedgerSHA256 == "" ||
		len(source.AuthorizationLedgerSignatureJSON) == 0 {
		t.Fatal("prerequisite scalar or ledger facts differ")
	}
}
