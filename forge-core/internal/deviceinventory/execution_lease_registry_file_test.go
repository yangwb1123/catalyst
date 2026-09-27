package deviceinventory

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/executionlease"
)

func TestPersistedExecutionLeaseRegistryClaimsAndReplaysWithoutNewToken(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "leases.json")
	tokenCalls := 0
	adapter, err := NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(path, owner, func() (string, error) {
		tokenCalls++
		return "token-a", nil
	})
	if err != nil {
		t.Fatal(err)
	}
	request := executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: executionlease.RequestDigest([]byte("body")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	candidates := []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 3, Generation: 2, HeartbeatSequence: 4,
	}}
	first, replayed, err := adapter.Claim(context.Background(), request, candidates)
	if err != nil || replayed || first.Grant.Epoch != 1 || first.Grant.FencingToken != "token-a" {
		t.Fatalf("first claim=%#v replayed=%v err=%v", first, replayed, err)
	}
	if tokenCalls != 1 {
		t.Fatalf("token calls after first claim=%d", tokenCalls)
	}
	replay, replayed, err := adapter.Claim(context.Background(), request, candidates)
	if err != nil || !replayed || replay.Grant != first.Grant {
		t.Fatalf("replay=%#v replayed=%v err=%v", replay, replayed, err)
	}
	if tokenCalls != 1 {
		t.Fatalf("replay generated a new token: calls=%d", tokenCalls)
	}
	request.RequestSHA256 = executionlease.RequestDigest([]byte("different"))
	if _, _, err := adapter.Claim(context.Background(), request, candidates); !errors.Is(err, executionlease.ErrIdempotencyConflict) {
		t.Fatalf("idempotency conflict=%v", err)
	}
}

func TestPersistedExecutionLeaseRegistrySkipsExpiredAndRejectsForeignImage(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "leases.json")
	adapter, err := NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(path, owner, func() (string, error) {
		return "token-b", nil
	})
	if err != nil {
		t.Fatal(err)
	}
	request := executionlease.ClaimRequest{
		ConversationID: "conversation-2", RunID: "run-2", AttemptID: "attempt-2",
		IdempotencyKey: "lease-key-00000002", RequestSHA256: executionlease.RequestDigest([]byte("body-2")),
		IssuedAtMS: 300_000, TTLMS: 30_000,
	}
	first, _, err := adapter.Claim(context.Background(), request, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}})
	if err != nil {
		t.Fatal(err)
	}
	request.ConversationID = "conversation-3"
	request.RunID = "run-3"
	request.AttemptID = "attempt-3"
	request.IdempotencyKey = "lease-key-00000003"
	request.RequestSHA256 = executionlease.RequestDigest([]byte("body-3"))
	request.IssuedAtMS = first.Grant.ExpiresAtMS + 1
	second, replayed, err := adapter.Claim(context.Background(), request, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 2, Generation: 1, HeartbeatSequence: 2,
	}})
	if err != nil || replayed || second.Grant.Epoch != 2 {
		t.Fatalf("expired replacement=%#v replayed=%v err=%v", second, replayed, err)
	}
}

func TestPersistedExecutionLeaseRegistryRenewsAndReplaysWithoutNewToken(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "leases.json")
	tokenCalls := 0
	adapter, err := NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(path, owner, func() (string, error) {
		tokenCalls++
		if tokenCalls == 1 {
			return "token-a", nil
		}
		return "token-b", nil
	})
	if err != nil {
		t.Fatal(err)
	}
	claim := executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	entry, replayed, err := adapter.Claim(context.Background(), claim, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}})
	if err != nil || replayed {
		t.Fatalf("claim entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	renew := executionlease.RenewRequest{
		ConversationID: claim.ConversationID, RunID: claim.RunID, AttemptID: claim.AttemptID,
		Proof: entry.Grant.Proof(), IdempotencyKey: "renew-key-00000001",
		RequestSHA256: executionlease.RequestDigest([]byte("renew")), IssuedAtMS: 210_000, TTLMS: 30_000,
	}
	renewed, replayed, err := adapter.Renew(context.Background(), renew)
	if err != nil || replayed || renewed.Grant.Epoch != 2 || renewed.Grant.FencingToken != "token-b" {
		t.Fatalf("renewed=%#v replayed=%v err=%v", renewed, replayed, err)
	}
	replay, replayed, err := adapter.Renew(context.Background(), renew)
	if err != nil || !replayed || replay.Grant != renewed.Grant {
		t.Fatalf("renew replay=%#v replayed=%v err=%v", replay, replayed, err)
	}
	if tokenCalls != 2 {
		t.Fatalf("token calls=%d", tokenCalls)
	}
}

func TestPersistedExecutionLeaseRegistryReleasesAndPreservesFencingHistory(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	root := t.TempDir()
	if err := os.Chmod(root, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "leases.json")
	tokenCalls := 0
	adapter, err := NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(path, owner, func() (string, error) {
		tokenCalls++
		return "token-" + string(rune('a'+tokenCalls-1)), nil
	})
	if err != nil {
		t.Fatal(err)
	}
	claim := executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "lease-key-00000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: 200_000, TTLMS: 30_000,
	}
	entry, replayed, err := adapter.Claim(context.Background(), claim, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
	}})
	if err != nil || replayed {
		t.Fatalf("claim entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	release := executionlease.ReleaseRequest{
		ConversationID: claim.ConversationID, RunID: claim.RunID, AttemptID: claim.AttemptID,
		Proof: entry.Grant.Proof(), IdempotencyKey: "release-key-00000001",
		RequestSHA256: executionlease.RequestDigest([]byte("release")), ReleasedAtMS: 210_000,
	}
	released, replayed, err := adapter.Release(context.Background(), release)
	if err != nil || replayed || released.ReleasedAtMS != 210_000 {
		t.Fatalf("released=%#v replayed=%v err=%v", released, replayed, err)
	}
	replay, replayed, err := adapter.Release(context.Background(), release)
	if err != nil || !replayed || replay.ReleasedAtMS != released.ReleasedAtMS {
		t.Fatalf("release replay=%#v replayed=%v err=%v", replay, replayed, err)
	}
	claimed, replayed, err := adapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-2", RunID: "run-2", AttemptID: "attempt-2",
		IdempotencyKey: "lease-key-00000002", RequestSHA256: executionlease.RequestDigest([]byte("claim-2")),
		IssuedAtMS: 210_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{
		DeviceID: "device-a", InstanceID: "runner-a", Revision: 2, Generation: 2, HeartbeatSequence: 2,
	}})
	if err != nil || replayed || claimed.Grant.Epoch != 2 {
		t.Fatalf("claim after release=%#v replayed=%v err=%v", claimed, replayed, err)
	}
	if tokenCalls != 2 {
		t.Fatalf("token calls=%d", tokenCalls)
	}
}
