package executionlease

import (
	"errors"
	"testing"
)

func TestClaimSelectsSortedCandidateAndAdvancesEpoch(t *testing.T) {
	request := ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("request")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	priorGrant, err := Issue("attempt-old", "runner-a", 1, "old-token", 190_000, 20_000)
	if err != nil {
		t.Fatal(err)
	}
	entries := []RegistryEntry{{
		ConversationID: "conversation-old", RunID: "run-old", AttemptID: "attempt-old",
		IdempotencyKey: "lease-old-0000001", RequestSHA256: RequestDigest([]byte("old")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: priorGrant,
	}}
	next, claimed, replayed, err := Claim(entries, request, []ClaimCandidate{
		{DeviceID: "device-b", InstanceID: "runner-b", Revision: 2, Generation: 1, HeartbeatSequence: 3},
		{DeviceID: "device-a", InstanceID: "runner-a", Revision: 2, Generation: 1, HeartbeatSequence: 3},
	}, "new-token")
	if err != nil || replayed {
		t.Fatalf("claim err=%v replayed=%v", err, replayed)
	}
	if claimed.DeviceID != "device-b" || claimed.InstanceID != "runner-b" || claimed.Grant.Epoch != 1 {
		t.Fatalf("claimed=%#v", claimed)
	}
	if len(next) != 2 || next[0].DeviceID != "device-a" || next[1].DeviceID != "device-b" {
		t.Fatalf("next=%#v", next)
	}
}

func TestClaimSkipsActiveTargetAndReplaysExactIdempotency(t *testing.T) {
	request := ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("request")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	grant, err := Issue("attempt-1", "runner-a", 2, "token-a", 190_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		IdempotencyKey: request.IdempotencyKey, RequestSHA256: request.RequestSHA256,
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	next, replayedEntry, replayed, err := Claim([]RegistryEntry{entry}, request, nil, "must-not-be-used")
	if err != nil || !replayed || replayedEntry.Grant != grant || len(next) != 1 {
		t.Fatalf("replay next=%#v entry=%#v replayed=%v err=%v", next, replayedEntry, replayed, err)
	}
	request.RequestSHA256 = RequestDigest([]byte("different"))
	if _, _, _, err := Claim([]RegistryEntry{entry}, request, nil, "must-not-be-used"); !errors.Is(err, ErrIdempotencyConflict) {
		t.Fatalf("conflict err=%v", err)
	}
}

func TestClaimRejectsReservedOnlyAndInvalidCandidate(t *testing.T) {
	request := ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("request")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	grant, err := Issue("attempt-old", "runner-a", 1, "token-a", 190_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: "conversation-old", RunID: "run-old", AttemptID: "attempt-old",
		IdempotencyKey: "lease-old-0000001", RequestSHA256: RequestDigest([]byte("old")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	_, _, _, err = Claim([]RegistryEntry{entry}, request, []ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}}, "new-token")
	if !errors.Is(err, ErrTargetReserved) {
		t.Fatalf("reserved err=%v", err)
	}
	_, _, _, err = Claim(nil, request, []ClaimCandidate{{DeviceID: "device-a"}}, "new-token")
	if !errors.Is(err, ErrInvalidClaimRequest) {
		t.Fatalf("invalid candidate err=%v", err)
	}
}

func TestRenewFencesCurrentLeaseAndReplaysWithoutAnotherToken(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-a", 1, "token-a", 190_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("claim")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	request := RenewRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Proof: grant.Proof(), IdempotencyKey: "renew-key-00000001",
		RequestSHA256: RequestDigest([]byte("renew")), IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	next, renewed, replayed, err := Renew([]RegistryEntry{entry}, request, "token-b")
	if err != nil || replayed || renewed.Grant.Epoch != 2 || renewed.Grant.FencingToken != "token-b" {
		t.Fatalf("renew next=%#v renewed=%#v replayed=%v err=%v", next, renewed, replayed, err)
	}
	if len(next) != 2 {
		t.Fatalf("renew entries=%#v", next)
	}
	_, replay, replayed, err := Renew(next, request, "must-not-be-used")
	if err != nil || !replayed || replay.Grant != renewed.Grant {
		t.Fatalf("renew replay=%#v replayed=%v err=%v", replay, replayed, err)
	}
	stale := request
	stale.IdempotencyKey = "renew-key-00000002"
	stale.RequestSHA256 = RequestDigest([]byte("stale"))
	if _, _, _, err := Renew(next, stale, "token-c"); !errors.Is(err, ErrLeaseStale) {
		t.Fatalf("stale renew err=%v", err)
	}
}

func TestRenewRejectsExpiredProof(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-a", 1, "token-a", 100_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("claim")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	request := RenewRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Proof: grant.Proof(), IdempotencyKey: "renew-key-00000001",
		RequestSHA256: RequestDigest([]byte("renew")), IssuedAtMS: 130_000, TTLMS: 30_000,
	}
	if _, _, _, err := Renew([]RegistryEntry{entry}, request, "token-b"); !errors.Is(err, ErrLeaseExpired) {
		t.Fatalf("expired renew err=%v", err)
	}
}

func TestReleasePreservesEpochFencingAndReplays(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-a", 1, "token-a", 190_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("claim")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	request := ReleaseRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Proof: grant.Proof(), IdempotencyKey: "release-key-00000001",
		RequestSHA256: RequestDigest([]byte("release")), ReleasedAtMS: 200_000,
	}
	next, released, replayed, err := Release([]RegistryEntry{entry}, request)
	if err != nil || replayed || released.ReleasedAtMS != request.ReleasedAtMS || len(next) != 1 {
		t.Fatalf("release next=%#v released=%#v replayed=%v err=%v", next, released, replayed, err)
	}
	if next[0].IsActive(200_000) || next[0].Grant != grant {
		t.Fatalf("release lost fencing history: %#v", next[0])
	}
	_, replay, replayed, err := Release(next, request)
	if err != nil || !replayed || replay.ReleasedAtMS != request.ReleasedAtMS {
		t.Fatalf("release replay=%#v replayed=%v err=%v", replay, replayed, err)
	}
	renew := RenewRequest{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		Proof: grant.Proof(), IdempotencyKey: "renew-after-release", RequestSHA256: RequestDigest([]byte("renew-after-release")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	if _, _, _, err := Renew(next, renew, "token-b"); !errors.Is(err, ErrLeaseReleased) {
		t.Fatalf("renew released err=%v", err)
	}
	if _, _, _, err := Claim(next, ClaimRequest{
		ConversationID: "conversation-2", RunID: "run-2", AttemptID: "attempt-2",
		IdempotencyKey: request.IdempotencyKey, RequestSHA256: RequestDigest([]byte("claim-collision")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}, []ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 2, Generation: 2, HeartbeatSequence: 2}}, "token-c"); !errors.Is(err, ErrIdempotencyConflict) {
		t.Fatalf("claim release-key collision err=%v", err)
	}
	if _, _, _, err := Renew(next, RenewRequest{
		ConversationID: request.ConversationID, RunID: request.RunID, AttemptID: request.AttemptID,
		Proof: grant.Proof(), IdempotencyKey: request.IdempotencyKey,
		RequestSHA256: RequestDigest([]byte("renew-collision")), IssuedAtMS: 200_000, TTLMS: 30_000,
	}, "token-b"); !errors.Is(err, ErrIdempotencyConflict) {
		t.Fatalf("renew release-key collision err=%v", err)
	}
	claimed, _, replayed, err := Claim(next, ClaimRequest{
		ConversationID: "conversation-2", RunID: "run-2", AttemptID: "attempt-2",
		IdempotencyKey: "lease-key-00000002", RequestSHA256: RequestDigest([]byte("claim-2")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}, []ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 2, Generation: 2, HeartbeatSequence: 2}}, "token-c")
	if err != nil || replayed || len(claimed) != 2 || claimed[len(claimed)-1].Grant.Epoch != 2 {
		t.Fatalf("claim after release entries=%#v replayed=%v err=%v", claimed, replayed, err)
	}
}

func TestReleaseRejectsStaleAndExpiredProofs(t *testing.T) {
	grant, err := Issue("attempt-1", "runner-a", 1, "token-a", 100_000, 30_000)
	if err != nil {
		t.Fatal(err)
	}
	entry := RegistryEntry{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: RequestDigest([]byte("claim")),
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1,
		HeartbeatSequence: 1, Grant: grant,
	}
	base := ReleaseRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Proof: grant.Proof(), IdempotencyKey: "release-key-00000001",
		RequestSHA256: RequestDigest([]byte("release")), ReleasedAtMS: 130_000,
	}
	if _, _, _, err := Release([]RegistryEntry{entry}, base); !errors.Is(err, ErrLeaseExpired) {
		t.Fatalf("expired release err=%v", err)
	}
	newGrant, err := grant.Renew(110_000, "token-b", 30_000)
	if err != nil {
		t.Fatal(err)
	}
	newEntry := entry
	newEntry.IdempotencyKey = "renew-key-00000001"
	newEntry.RequestSHA256 = RequestDigest([]byte("renew"))
	newEntry.Grant = newGrant
	stale := base
	stale.IdempotencyKey = "release-key-00000002"
	stale.RequestSHA256 = RequestDigest([]byte("stale"))
	stale.ReleasedAtMS = 115_000
	if _, _, _, err := Release([]RegistryEntry{entry, newEntry}, stale); !errors.Is(err, ErrLeaseStale) {
		t.Fatalf("stale release err=%v", err)
	}
}
