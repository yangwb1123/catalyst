package outputbindingstore

import (
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/outputbinding"
)

func TestAppendRejectsCandidateAttemptChallengeAndBindingReplayBeforeWrite(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*outputbinding.AgentOutputReceipt, outputbinding.AgentOutputReceipt)
		want   string
	}{
		{"attempt", replayAttemptOnly, "attempt is not strictly increasing"},
		{"challenge", replayChallengeOnly, "reuses a prior challenge"},
		{"binding", replayCompletePreflight, "reuses a prior challenge"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			store := New(root)
			first, err := store.Append(receiptDraft(t, "first"))
			if err != nil {
				t.Fatal(err)
			}
			before, err := os.ReadFile(Path(root))
			if err != nil {
				t.Fatal(err)
			}
			draft := receiptDraft(t, "second")
			test.mutate(&draft, first)
			resealDraftPreflight(t, &draft)
			assertReplayShape(t, test.name, draft, first)
			if _, err = store.Append(draft); err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("replayed %s error = %v", test.name, err)
			}
			assertLedgerBytes(t, Path(root), before)
		})
	}
}

func assertReplayShape(t *testing.T, name string, draft,
	first outputbinding.AgentOutputReceipt) {
	t.Helper()
	sameChallenge := draft.Challenge == first.Challenge
	sameBinding := draft.BindingSHA256 == first.BindingSHA256
	switch name {
	case "attempt":
		if sameChallenge || sameBinding {
			t.Fatal("attempt replay fixture also replayed challenge or binding")
		}
	case "challenge":
		if !sameChallenge || sameBinding {
			t.Fatal("challenge replay fixture did not isolate the challenge")
		}
	case "binding":
		if !sameChallenge || !sameBinding {
			t.Fatal("complete preflight replay fixture did not replay the exact binding")
		}
	}
}

func replayAttemptOnly(draft *outputbinding.AgentOutputReceipt,
	first outputbinding.AgentOutputReceipt) {
	draft.RunID = first.RunID
	draft.Attempt = first.Attempt
}

func replayChallengeOnly(draft *outputbinding.AgentOutputReceipt,
	first outputbinding.AgentOutputReceipt) {
	draft.Challenge = first.Challenge
}

func replayCompletePreflight(draft *outputbinding.AgentOutputReceipt,
	first outputbinding.AgentOutputReceipt) {
	draft.RunID = first.RunID
	draft.Workflow = first.Workflow
	draft.Phase = first.Phase
	draft.Attempt = first.Attempt
	draft.Challenge = first.Challenge
	draft.PromptContextSHA256 = first.PromptContextSHA256
	draft.SourceBeforeSHA256 = first.SourceBeforeSHA256
	draft.ArtifactInputs = first.ArtifactInputs
	draft.ArtifactInputsSHA256 = first.ArtifactInputsSHA256
	draft.RuntimePolicy = first.RuntimePolicy
	draft.LocalRuntimePolicySHA256 = first.LocalRuntimePolicySHA256
}

func resealDraftPreflight(t *testing.T, draft *outputbinding.AgentOutputReceipt) {
	t.Helper()
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
	draft.BindingSHA256 = preflight.BindingSHA256
}
