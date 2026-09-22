package deviceapproval

import (
	"math"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
)

func testOwner() deviceidentity.Owner {
	return deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
}

func testPendingState(t *testing.T) State {
	t.Helper()
	state, err := NewPending(testDeviceID(), testOwner(), "key-1", testDigest('a'))
	if err != nil {
		t.Fatalf("NewPending() error = %v", err)
	}
	return state
}

func testDeviceID() string { return "device-1" }

func testDigest(character byte) string {
	result := make([]byte, DigestHexBytes)
	for index := range result {
		result[index] = character
	}
	return string(result)
}

func requestFor(state State, action Action) Request {
	return Request{Current: state, Action: action, Owner: state.Owner, DeviceID: state.DeviceID}
}

func TestPendingApprovedRevokedLifecycleIsStrictAndTerminal(t *testing.T) {
	pending := testPendingState(t)
	approvedTransition, err := Apply(requestFor(pending, ActionApprove))
	if err != nil {
		t.Fatalf("pending approval error = %v", err)
	}
	if approvedTransition.Next.ApprovalState != ApprovalApproved || approvedTransition.Next.KeyGeneration != 1 {
		t.Fatalf("approved transition = %#v", approvedTransition.Next)
	}
	revokedTransition, err := Apply(requestFor(approvedTransition.Next, ActionRevoke))
	if err != nil {
		t.Fatalf("approved revoke error = %v", err)
	}
	if revokedTransition.Next.ApprovalState != ApprovalRevoked {
		t.Fatalf("revoked transition = %#v", revokedTransition.Next)
	}
	for _, action := range []Action{ActionApprove, ActionRevoke, ActionRotateKey} {
		req := requestFor(revokedTransition.Next, action)
		if action == ActionRotateKey {
			req.NextKeyID = "key-2"
			req.NextPublicKeySHA256 = testDigest('b')
		}
		if _, err := Apply(req); err != ErrRevocationTerminal {
			t.Errorf("revoked %s error = %v, want %v", action, err, ErrRevocationTerminal)
		}
	}
}

func TestPendingCanBeRevokedWithoutBecomingApproved(t *testing.T) {
	transition, err := Apply(requestFor(testPendingState(t), ActionRevoke))
	if err != nil {
		t.Fatalf("pending revoke error = %v", err)
	}
	if transition.Next.ApprovalState != ApprovalRevoked {
		t.Fatalf("pending revoke state = %#v", transition.Next)
	}
}

func TestRotationIncrementsGenerationAndPreservesIdentityAndApproval(t *testing.T) {
	current := testPendingState(t)
	current.ApprovalState = ApprovalApproved
	request := requestFor(current, ActionRotateKey)
	request.NextKeyID = "key-2"
	request.NextPublicKeySHA256 = testDigest('b')
	transition, err := Apply(request)
	if err != nil {
		t.Fatalf("rotation error = %v", err)
	}
	want := current
	want.KeyID = request.NextKeyID
	want.PublicKeySHA256 = request.NextPublicKeySHA256
	want.KeyGeneration++
	if !reflect.DeepEqual(transition.Next, want) {
		t.Fatalf("rotated state = %#v, want %#v", transition.Next, want)
	}
	if transition.Next.DeviceID != current.DeviceID || transition.Next.Owner != current.Owner {
		t.Fatal("rotation changed immutable device identity")
	}
	if transition.Next.ApprovalState != ApprovalApproved {
		t.Fatal("rotation changed approval state")
	}
	if !transition.PreviewOnly || !transition.Authority.OwnerBindingMatched || transition.Authority.OwnerAuthenticated ||
		transition.Authority.Persisted || transition.Authority.CredentialIssued || transition.Authority.InventoryAuthoritative ||
		transition.Authority.ExecutionAuthorized {
		t.Fatalf("rotation authority boundary = %#v", transition)
	}
}

func TestRotationFromPendingPreservesPendingState(t *testing.T) {
	current := testPendingState(t)
	request := requestFor(current, ActionRotateKey)
	request.NextKeyID = "key-2"
	request.NextPublicKeySHA256 = testDigest('b')
	transition, err := Apply(request)
	if err != nil {
		t.Fatalf("pending rotation error = %v", err)
	}
	if transition.Next.ApprovalState != ApprovalPending || transition.Next.KeyGeneration != 2 {
		t.Fatalf("pending rotation state = %#v", transition.Next)
	}
}

func TestOwnerAndDeviceDriftAreRejected(t *testing.T) {
	state := testPendingState(t)
	ownerDrift := requestFor(state, ActionApprove)
	ownerDrift.Owner.Subject = "other-user"
	if _, err := Apply(ownerDrift); err != ErrOwnerMismatch {
		t.Fatalf("owner drift error = %v, want %v", err, ErrOwnerMismatch)
	}
	deviceDrift := requestFor(state, ActionApprove)
	deviceDrift.DeviceID = "other-device"
	if _, err := Apply(deviceDrift); err != ErrDeviceMismatch {
		t.Fatalf("device drift error = %v, want %v", err, ErrDeviceMismatch)
	}
}

func TestUnknownStateAndActionFailClosed(t *testing.T) {
	state := testPendingState(t)
	state.ApprovalState = "unknown"
	if _, err := Apply(requestFor(state, ActionApprove)); err != ErrUnknownApprovalState {
		t.Fatalf("unknown approval state error = %v, want %v", err, ErrUnknownApprovalState)
	}
	state = testPendingState(t)
	if _, err := Apply(requestFor(state, Action("reset"))); err != ErrUnknownAction {
		t.Fatalf("unknown action error = %v, want %v", err, ErrUnknownAction)
	}
}

func TestRotationRejectsUnchangedKeyInvalidKeyAndOverflow(t *testing.T) {
	state := testPendingState(t)
	unchanged := requestFor(state, ActionRotateKey)
	unchanged.NextKeyID = state.KeyID
	unchanged.NextPublicKeySHA256 = state.PublicKeySHA256
	if _, err := Apply(unchanged); err != ErrRotationKeyUnchanged {
		t.Fatalf("unchanged rotation error = %v, want %v", err, ErrRotationKeyUnchanged)
	}
	invalid := requestFor(state, ActionRotateKey)
	invalid.NextKeyID = "key-2"
	invalid.NextPublicKeySHA256 = "not-a-digest"
	if _, err := Apply(invalid); err != ErrInvalidRotationKey {
		t.Fatalf("invalid rotation error = %v, want %v", err, ErrInvalidRotationKey)
	}
	overflow := state
	overflow.KeyGeneration = math.MaxUint64
	overflowRequest := requestFor(overflow, ActionRotateKey)
	overflowRequest.NextKeyID = "key-2"
	overflowRequest.NextPublicKeySHA256 = testDigest('b')
	if _, err := Apply(overflowRequest); err != ErrKeyGenerationOverflow {
		t.Fatalf("overflow rotation error = %v, want %v", err, ErrKeyGenerationOverflow)
	}
}

func TestTransitionsRejectUnexpectedRotationMaterial(t *testing.T) {
	for _, action := range []Action{ActionApprove, ActionRevoke} {
		request := requestFor(testPendingState(t), action)
		request.NextKeyID = "key-2"
		if _, err := Apply(request); err != ErrUnexpectedRotationKey {
			t.Errorf("%s with rotation material error = %v, want %v", action, err, ErrUnexpectedRotationKey)
		}
	}
}
