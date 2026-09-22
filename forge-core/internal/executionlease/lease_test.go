package executionlease

import (
	"math"
	"testing"
)

const testTTL uint64 = 10_000

func testGrant(t *testing.T) LeaseGrant {
	t.Helper()
	grant, err := Issue("attempt-1", "runner-1", 1, "fence-1", 100, testTTL)
	if err != nil {
		t.Fatalf("issue grant: %v", err)
	}
	return grant
}

func completedDisposition() TerminalDisposition {
	return TerminalDisposition{Kind: "completed", ReceiptSHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
}

func TestGrantBindsIdentityAndActiveWindow(t *testing.T) {
	grant := testGrant(t)
	if grant.V != ExecutionLeaseABIVersion || !grant.IsActive(100) || !grant.IsActive(10_099) || grant.IsActive(10_100) {
		t.Fatalf("grant window is incorrect: %#v", grant)
	}
	if err := grant.ValidateProof(grant.Proof(), 10_099); err != nil {
		t.Fatalf("current proof rejected: %v", err)
	}
}

func TestRenewFencesTheOldProof(t *testing.T) {
	grant := testGrant(t)
	state, err := NewState(grant)
	if err != nil {
		t.Fatalf("new state: %v", err)
	}
	oldProof := state.Grant().Proof()
	if err := state.Renew(1_000, "fence-2", testTTL); err != nil {
		t.Fatalf("renew: %v", err)
	}
	if state.Grant().Epoch != 2 || state.Grant().FencingToken != "fence-2" {
		t.Fatalf("renewed grant=%#v", state.Grant())
	}
	if _, err := state.SubmitTerminal(oldProof, completedDisposition(), 2_000); err != ErrEpochMismatch {
		t.Fatalf("old proof error=%v, want %v", err, ErrEpochMismatch)
	}
}

func TestProofBindingErrorsHaveStablePrecedence(t *testing.T) {
	grant := testGrant(t)
	proof := grant.Proof()
	proof.AttemptID = "foreign-attempt"
	if err := grant.ValidateProof(proof, 200); err != ErrAttemptMismatch {
		t.Fatalf("attempt error=%v, want %v", err, ErrAttemptMismatch)
	}
	proof = grant.Proof()
	proof.TargetID = "foreign-target"
	if err := grant.ValidateProof(proof, 200); err != ErrTargetMismatch {
		t.Fatalf("target error=%v, want %v", err, ErrTargetMismatch)
	}
	proof = grant.Proof()
	proof.FencingToken = "foreign-token"
	if err := grant.ValidateProof(proof, 200); err != ErrFencingTokenMismatch {
		t.Fatalf("token error=%v, want %v", err, ErrFencingTokenMismatch)
	}
}

func TestExpiredLeaseRejectsRenewAndTerminal(t *testing.T) {
	grant := testGrant(t)
	state, err := NewState(grant)
	if err != nil {
		t.Fatalf("new state: %v", err)
	}
	if err := state.Renew(10_100, "fence-2", testTTL); err != ErrLeaseExpired {
		t.Fatalf("expired renew error=%v, want %v", err, ErrLeaseExpired)
	}
	if _, err := state.SubmitTerminal(grant.Proof(), completedDisposition(), 10_100); err != ErrLeaseExpired {
		t.Fatalf("expired terminal error=%v, want %v", err, ErrLeaseExpired)
	}
}

func TestTerminalReplayReturnsOriginalAndChangedOutcomeConflicts(t *testing.T) {
	grant := testGrant(t)
	state, err := NewState(grant)
	if err != nil {
		t.Fatalf("new state: %v", err)
	}
	first, err := state.SubmitTerminal(grant.Proof(), completedDisposition(), 200)
	if err != nil || first.Replayed {
		t.Fatalf("first terminal=%#v err=%v", first, err)
	}
	replay, err := state.SubmitTerminal(grant.Proof(), completedDisposition(), 20_000)
	if err != nil || !replay.Replayed || replay.Receipt != first.Receipt {
		t.Fatalf("replay=%#v err=%v first=%#v", replay, err, first)
	}
	if _, err := state.SubmitTerminal(grant.Proof(), TerminalDisposition{Kind: "failed", Reason: "late"}, 20_000); err != ErrTerminalAlreadyRecorded {
		t.Fatalf("changed outcome error=%v, want %v", err, ErrTerminalAlreadyRecorded)
	}
}

func TestUncertainIsTerminalAndNeverAutomaticRetry(t *testing.T) {
	grant := testGrant(t)
	state, err := NewState(grant)
	if err != nil {
		t.Fatalf("new state: %v", err)
	}
	uncertain := TerminalDisposition{Kind: "uncertain", Reason: "transport ended after effect boundary"}
	result, err := state.SubmitTerminal(grant.Proof(), uncertain, 200)
	if err != nil || !result.Receipt.Disposition.IsUncertain() {
		t.Fatalf("uncertain result=%#v err=%v", result, err)
	}
	if _, ok := state.Terminal(); !ok {
		t.Fatal("uncertain result was not terminal")
	}
	if err := state.Renew(201, "fence-2", testTTL); err != ErrTerminalAlreadyRecorded {
		t.Fatalf("renew after uncertain error=%v, want %v", err, ErrTerminalAlreadyRecorded)
	}
}

func TestBoundsAndMonotonicTimeFailClosed(t *testing.T) {
	if _, err := Issue("attempt", "target", 0, "token", 1, testTTL); err != ErrInvalidEpoch {
		t.Fatalf("zero epoch error=%v, want %v", err, ErrInvalidEpoch)
	}
	if _, err := Issue("attempt", "target", 1, "token", math.MaxUint64, testTTL); err != ErrTimeOverflow {
		t.Fatalf("time overflow error=%v, want %v", err, ErrTimeOverflow)
	}
	grant := testGrant(t)
	if _, err := grant.Renew(99, "fence-2", testTTL); err != ErrTimeWentBackwards {
		t.Fatalf("clock rollback error=%v, want %v", err, ErrTimeWentBackwards)
	}
	if _, err := grant.Renew(200, "fence-1", testTTL); err != ErrFencingTokenReused {
		t.Fatalf("token reuse error=%v, want %v", err, ErrFencingTokenReused)
	}
	maxEpoch, err := Issue("attempt", "target", math.MaxUint64, "token", 1, testTTL)
	if err != nil {
		t.Fatalf("max epoch grant: %v", err)
	}
	if _, err := maxEpoch.Renew(200, "fence-2", testTTL); err != ErrEpochOverflow {
		t.Fatalf("epoch overflow error=%v, want %v", err, ErrEpochOverflow)
	}
}

func TestDispositionAndGrantValidationRejectMalformedValues(t *testing.T) {
	cases := []struct {
		name string
		got  error
		want LeaseError
	}{
		{name: "invalid completed digest", got: (TerminalDisposition{Kind: "completed", ReceiptSHA256: "bad"}).Validate(), want: ErrInvalidDigest},
		{name: "invalid failed reason", got: (TerminalDisposition{Kind: "failed"}).Validate(), want: ErrInvalidReason},
		{name: "invalid uncertain reason", got: (TerminalDisposition{Kind: "uncertain", Reason: "bad\x00reason"}).Validate(), want: ErrInvalidReason},
		{name: "invalid identity", got: (LeaseProof{AttemptID: " bad", TargetID: "target", Epoch: 1, FencingToken: "token"}).Validate(), want: ErrInvalidIdentity},
		{name: "invalid token", got: (LeaseProof{AttemptID: "attempt", TargetID: "target", Epoch: 1, FencingToken: "bad\n"}).Validate(), want: ErrInvalidFencingToken},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			if test.got != test.want {
				t.Fatalf("error=%v, want %v", test.got, test.want)
			}
		})
	}
}

func TestLeaseErrorCodeIsTheStableMachineCode(t *testing.T) {
	if ErrEpochMismatch.Code() != "epoch_mismatch" || ErrEpochMismatch.Error() != ErrEpochMismatch.Code() {
		t.Fatalf("epoch mismatch code=%q error=%q", ErrEpochMismatch.Code(), ErrEpochMismatch.Error())
	}
}
