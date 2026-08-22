package outputbindingstore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/outputbinding"
)

func TestAppendBuildsStrictDurableChain(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	first, err := store.Append(receiptDraft(t, "first"))
	if err != nil {
		t.Fatal(err)
	}
	second, err := store.Append(receiptDraft(t, "second"))
	if err != nil {
		t.Fatal(err)
	}
	if first.LedgerSequence != 1 || first.PriorReceiptSHA256 != nil ||
		second.LedgerSequence != 2 || second.PriorReceiptSHA256 == nil ||
		*second.PriorReceiptSHA256 != first.ReceiptSHA256 {
		t.Fatalf("chain fields = first %#v second %#v", first, second)
	}
	loaded, err := store.Load()
	if err != nil || len(loaded) != 2 || loaded[1].ReceiptSHA256 != second.ReceiptSHA256 {
		t.Fatalf("Load = %#v, %v", loaded, err)
	}
	info, err := os.Stat(Path(root))
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("ledger mode = %v, %v", info, err)
	}
	anchorInfo, err := os.Stat(ReceiptAnchorPath(root))
	if err != nil || anchorInfo.Mode().Perm() != 0o600 {
		t.Fatalf("receipt anchor mode = %v, %v", anchorInfo, err)
	}
	dirInfo, err := os.Stat(filepath.Join(root, ".forge"))
	if err != nil || dirInfo.Mode().Perm() != 0o700 {
		t.Fatalf("ledger directory mode = %v, %v", dirInfo, err)
	}
}

func TestAppendRejectsCorruptionAndUnsafeLeaf(t *testing.T) {
	t.Run("truncated", func(t *testing.T) {
		root := t.TempDir()
		partial := []byte(`{"partial":`)
		if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(Path(root), partial, 0o640); err != nil {
			t.Fatal(err)
		}
		if _, err := New(root).Append(receiptDraft(t, "blocked")); err == nil ||
			!strings.Contains(err.Error(), "truncated final line") {
			t.Fatalf("truncated ledger error = %v", err)
		}
		assertLedgerImage(t, Path(root), partial, 0o640)
	})
	t.Run("symlink", func(t *testing.T) {
		root, outside := t.TempDir(), filepath.Join(t.TempDir(), "outside")
		if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, Path(root)); err != nil {
			t.Fatal(err)
		}
		if _, err := New(root).Append(receiptDraft(t, "blocked")); err == nil {
			t.Fatal("symlink ledger was accepted")
		}
		data, _ := os.ReadFile(outside)
		if string(data) != "sentinel" {
			t.Fatalf("outside target mutated: %q", data)
		}
	})
}

func TestAppendRejectsCallerControlledChainFields(t *testing.T) {
	draft := receiptDraft(t, "bad")
	draft.LedgerSequence = 1
	if _, err := New(t.TempDir()).Append(draft); err == nil || !strings.Contains(err.Error(), "must not set") {
		t.Fatalf("caller chain field error = %v", err)
	}
}

func receiptDraft(t *testing.T, label string) outputbinding.AgentOutputReceipt {
	t.Helper()
	empty, err := outputbinding.SealManifest(nil)
	if err != nil {
		t.Fatal(err)
	}
	digest := func(value string) string { return outputbinding.SHA256([]byte(value)) }
	policy, err := outputbinding.SealRuntimePolicy(outputbinding.RuntimePolicyBinding{
		ADR: false, Agent: "reviewer", BuildHalt: false, DesignDepth: "full",
		DiscoverDepth: "full", EvolveAuthority: "auto-act", EvolveDepth: "thorough",
		Executor: "test-executor", Gates: []string{"test"}, Lifecycle: "mvp",
		Materiality: "L4", Mode: "engineering", Model: "opus",
		OutputBindingContract: outputbinding.LocalDigestProfile, Phase: "reviewer", Readonly: true,
		ReviewDepth: "full", Reviewer: true, Stage: "build",
		VerdictContract: "reviewer_v2", WorkflowSHA256: digest("workflow"),
	})
	if err != nil {
		t.Fatal(err)
	}
	preflight, err := outputbinding.SealPreflight(outputbinding.PreflightBinding{
		ArtifactInputsSHA256: empty.ManifestSHA256, Attempt: 1,
		Challenge: digest("challenge-" + label), LocalRuntimePolicySHA256: policy.BindingSHA256,
		Phase: "reviewer", PromptContextSHA256: digest("prompt"), RunID: "run-" + label,
		SourceBeforeSHA256: digest("source"), Workflow: "build",
		WorkflowSHA256: policy.WorkflowSHA256,
	})
	if err != nil {
		t.Fatal(err)
	}
	verdict := "APPROVE"
	return outputbinding.AgentOutputReceipt{
		Agent: "reviewer", ArtifactInputs: empty, ArtifactInputsSHA256: empty.ManifestSHA256,
		ArtifactOutputs: empty, ArtifactOutputsSHA256: empty.ManifestSHA256,
		Attempt: 1, BindingSHA256: preflight.BindingSHA256, Challenge: preflight.Challenge,
		Executor: "test-executor", FinalPromptSHA256: digest("final-prompt"),
		LocalRuntimePolicySHA256: policy.BindingSHA256, Model: "opus",
		ObservedAtUnixMS: 1, Phase: "reviewer", PromptContextSHA256: digest("prompt"),
		RawOutputBytes: int64(len(label)), RawOutputSHA256: digest(label), RunID: "run-" + label,
		RuntimePolicy: policy, SemanticOutputBytes: int64(len(label)),
		SemanticOutputSHA256: digest(label), SourceAfterSHA256: digest("source"),
		SourceBeforeSHA256: digest("source"),
		SourceRevision:     "git-sha1:" + strings.Repeat("a", 40), Verdict: &verdict,
		Workflow: "build",
	}
}
