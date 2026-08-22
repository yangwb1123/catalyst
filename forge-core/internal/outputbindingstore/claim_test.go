package outputbindingstore

import (
	"bytes"
	"os"
	"testing"

	"forgeos/forge-core/internal/outputbinding"
)

func TestPreflightClaimsPersistInterruptedAttempts(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	first := claimFixture(t, "run", "phase", 1, "first")
	second := claimFixture(t, "run", "phase", 2, "second")
	if err := store.ClaimPreflight(first); err != nil {
		t.Fatal(err)
	}
	if err := New(root).ClaimPreflight(second); err != nil {
		t.Fatal(err)
	}
	claims, err := New(root).LoadPreflightClaims()
	if err != nil || len(claims) != 2 || claims[0] != first || claims[1] != second {
		t.Fatalf("claims = %#v, err=%v", claims, err)
	}
}

func TestPreflightClaimValidPrefixRollbackFailsClosed(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if err := store.ClaimPreflight(claimFixture(t, "run", "phase", 1, "first")); err != nil {
		t.Fatal(err)
	}
	firstImage, err := os.ReadFile(ClaimPath(root))
	if err != nil {
		t.Fatal(err)
	}
	if err := store.ClaimPreflight(claimFixture(t, "run", "phase", 2, "second")); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(ClaimPath(root), firstImage, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).LoadPreflightClaims(); err == nil {
		t.Fatal("valid-prefix claim rollback was accepted")
	}
}

func TestPreflightClaimsRejectReplayWithoutChangingBytes(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	first := claimFixture(t, "run", "phase", 1, "first")
	if err := store.ClaimPreflight(first); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(ClaimPath(root))
	if err != nil {
		t.Fatal(err)
	}
	for _, replay := range []outputbinding.PreflightBinding{
		claimFixture(t, "run", "phase", 1, "other"),
		first,
		claimWithReusedChallenge(t, first),
	} {
		if err := New(root).ClaimPreflight(replay); err == nil {
			t.Fatal("replayed preflight claim was accepted")
		}
		after, readErr := os.ReadFile(ClaimPath(root))
		if readErr != nil || !bytes.Equal(before, after) {
			t.Fatalf("rejected claim changed journal: %v", readErr)
		}
	}
}

func TestReceiptMustReferenceExactPreflightClaim(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	draft := receiptDraft(t, "claimed")
	preflight, err := outputbinding.SealPreflight(outputbinding.PreflightBinding{
		ArtifactInputsSHA256: draft.ArtifactInputsSHA256, Attempt: draft.Attempt,
		Challenge: draft.Challenge, LocalRuntimePolicySHA256: draft.LocalRuntimePolicySHA256,
		Phase: draft.Phase, PromptContextSHA256: draft.PromptContextSHA256,
		RunID: draft.RunID, SourceBeforeSHA256: draft.SourceBeforeSHA256,
		Workflow: draft.Workflow, WorkflowSHA256: draft.RuntimePolicy.WorkflowSHA256,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := store.ClaimPreflight(preflight); err != nil {
		t.Fatal(err)
	}
	receipt, err := store.Append(draft)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.RequireReceiptClaim(receipt); err != nil {
		t.Fatal(err)
	}
	other := receipt
	other.Challenge = outputbinding.SHA256([]byte("other"))
	other.BindingSHA256 = outputbinding.SHA256([]byte("other binding"))
	if err := store.RequireReceiptClaim(other); err == nil {
		t.Fatal("receipt without exact claim was accepted")
	}
}

func claimFixture(t *testing.T, runID, phase string, attempt int64, label string) outputbinding.PreflightBinding {
	t.Helper()
	value, err := outputbinding.SealPreflight(outputbinding.PreflightBinding{
		ArtifactInputsSHA256: outputbinding.SHA256([]byte("artifacts")), Attempt: attempt,
		Challenge:                outputbinding.SHA256([]byte("challenge-" + label)),
		LocalRuntimePolicySHA256: outputbinding.SHA256([]byte("policy")), Phase: phase,
		PromptContextSHA256: outputbinding.SHA256([]byte("prompt")), RunID: runID,
		SourceBeforeSHA256: outputbinding.SHA256([]byte("source")), Workflow: "build",
		WorkflowSHA256: outputbinding.SHA256([]byte("workflow")),
	})
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func claimWithReusedChallenge(t *testing.T,
	first outputbinding.PreflightBinding) outputbinding.PreflightBinding {
	t.Helper()
	second := claimFixture(t, "other-run", "other-phase", 1, "other")
	second.Challenge = first.Challenge
	sealed, err := outputbinding.SealPreflight(second)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}
