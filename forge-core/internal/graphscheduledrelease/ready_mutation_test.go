package graphscheduledrelease

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

func TestReadyControlRejectsProgressDecisionAndSelectionDrift(t *testing.T) {
	base, _ := validReadySuccessorFixture(t, "")
	cases := []struct {
		name   string
		mutate func(*ReadyReleaseControl)
	}{
		{"stale progress digest", func(v *ReadyReleaseControl) {
			v.ProgressSnapshot.Nodes[0].NodeID = "other"
		}},
		{"non ready", func(v *ReadyReleaseControl) {
			selected := &v.ProgressSnapshot.Nodes[v.ScheduledContract.Node.ExecutionOrdinal]
			selected.LifecycleStatus = readyStringPointerTest("claimed")
			refreshReadyDecisionTest(t, v)
		}},
		{"supplied decision", func(v *ReadyReleaseControl) {
			v.ReconcileDecision.NextNodeID = readyStringPointerTest("other")
		}},
		{"progress node", func(v *ReadyReleaseControl) {
			selected := &v.ProgressSnapshot.Nodes[v.ScheduledContract.Node.ExecutionOrdinal]
			selected.NodeID = "other"
			refreshReadyDecisionTest(t, v)
		}},
		{"candidate identity", func(v *ReadyReleaseControl) {
			selected := &v.ProgressSnapshot.Nodes[v.ScheduledContract.Node.ExecutionOrdinal]
			digest := strings.Repeat("0", 64)
			selected.CandidateID = readyStringPointerTest("scheduled-node-contract-" + digest)
			selected.CandidateSHA256 = readyStringPointerTest(digest)
			refreshReadyDecisionTest(t, v)
		}},
		{"candidate scope", func(v *ReadyReleaseControl) {
			v.ScheduledContract.ContractScope = "schedule_initial_node_only"
		}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := cloneReadyControlTest(t, base)
			test.mutate(&value)
			assertReadyControlRejectedTest(t, &value)
		})
	}
}

func TestReadyControlRejectsEveryNonReadyDisposition(t *testing.T) {
	base, _ := validReadySuccessorFixture(t, "")
	cases := []struct {
		name    string
		status  string
		outcome string
	}{
		{"claimed", "claimed", ""},
		{"quarantined", "quarantined", ""},
		{"adjudicated", "adjudicated", ""},
		{"failed", "terminalized", "failed"},
		{"failed uncertain", "terminalized", "failed_uncertain"},
		{"completed", "terminalized", "completed"},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := cloneReadyControlTest(t, base)
			setReadySelectedLifecycleTest(&value, test.status, test.outcome)
			refreshReadyDecisionTest(t, &value)
			assertReadyControlRejectedTest(t, &value)
		})
	}
	t.Run("incompatible progress", func(t *testing.T) {
		value, _ := validReadyZeroDirectSuccessorFixture(t)
		future := &value.ProgressSnapshot.Nodes[2]
		digest := strings.Repeat("8", 64)
		future.CandidateID = readyStringPointerTest("scheduled-node-contract-" + digest)
		future.CandidateSHA256 = readyStringPointerTest(digest)
		refreshReadyDecisionTest(t, &value)
		assertReadyControlRejectedTest(t, &value)
	})
}

func setReadySelectedLifecycleTest(value *ReadyReleaseControl, status, outcome string) {
	selected := &value.ProgressSnapshot.Nodes[value.ScheduledContract.Node.ExecutionOrdinal]
	selected.LifecycleStatus = readyStringPointerTest(status)
	if status == "terminalized" {
		selected.TerminalOutcome = readyStringPointerTest(outcome)
		selected.TerminalReceiptSHA256 = readyStringPointerTest(strings.Repeat("7", 64))
	}
}

func TestReadyControlRejectsClosureAndArtifactDrift(t *testing.T) {
	base, _ := validReadySuccessorFixture(t, "")
	cases := []struct {
		name   string
		mutate func(*ReadyReleaseControl)
	}{
		{"reordered", func(v *ReadyReleaseControl) {
			v.DirectPredecessorReceipts[0], v.DirectPredecessorReceipts[1] =
				v.DirectPredecessorReceipts[1], v.DirectPredecessorReceipts[0]
		}},
		{"missing", func(v *ReadyReleaseControl) {
			v.DirectPredecessorReceipts = v.DirectPredecessorReceipts[:1]
		}},
		{"extra", func(v *ReadyReleaseControl) {
			v.DirectPredecessorReceipts = append(
				v.DirectPredecessorReceipts, v.DirectPredecessorReceipts[0],
			)
		}},
		{"receipt dispatch", func(v *ReadyReleaseControl) {
			v.DirectPredecessorReceipts[0].DispatchID = "different-dispatch"
			v.DirectPredecessorReceipts[0] = resignReadyReceiptTest(t, v.DirectPredecessorReceipts[0])
		}},
		{"receipt lane", func(v *ReadyReleaseControl) {
			v.DirectPredecessorReceipts[0].ProjectLaneSHA256 = strings.Repeat("0", 64)
			v.DirectPredecessorReceipts[0] = resignReadyReceiptTest(t, v.DirectPredecessorReceipts[0])
		}},
		{"progress receipt", func(v *ReadyReleaseControl) {
			v.ProgressSnapshot.Nodes[0].TerminalReceiptSHA256 = readyStringPointerTest(strings.Repeat("0", 64))
			refreshReadyDecisionTest(t, v)
		}},
		{"progress request", func(v *ReadyReleaseControl) {
			digest := strings.Repeat("9", 64)
			v.ProgressSnapshot.Nodes[0].ProviderRequestID =
				readyStringPointerTest("scheduled-node-provider-request-" + digest)
			v.ProgressSnapshot.Nodes[0].PreparedRequestSHA256 = readyStringPointerTest(digest)
			refreshReadyDecisionTest(t, v)
		}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := cloneReadyControlTest(t, base)
			test.mutate(&value)
			assertReadyControlRejectedTest(t, &value)
		})
	}
	assertReadyArtifactMutationsRejected(t)
}

func assertReadyArtifactMutationsRejected(t *testing.T) {
	t.Helper()
	content, _ := validReadySuccessorFixture(t, "verified predecessor output")
	t.Run("missing content artifact", func(t *testing.T) {
		value := cloneReadyControlTest(t, content)
		value.PredecessorContentArtifact = nil
		assertReadyControlRejectedTest(t, &value)
	})
	t.Run("artifact output", func(t *testing.T) {
		value := cloneReadyControlTest(t, content)
		artifact := *value.PredecessorContentArtifact
		artifact.OutputText = "different output"
		artifact = resignReadyArtifactTest(t, artifact)
		value.PredecessorContentArtifact = &artifact
		assertReadyControlRejectedTest(t, &value)
	})
	t.Run("artifact without content", func(t *testing.T) {
		value, _ := validReadySuccessorFixture(t, "")
		value.PredecessorContentArtifact = content.PredecessorContentArtifact
		assertReadyControlRejectedTest(t, &value)
	})
}

func TestReadyControlRejectsProviderAndPolicyDrift(t *testing.T) {
	base, _ := validReadyInitialFixture(t)
	cases := []struct {
		name   string
		mutate func(*ReadyReleaseControl)
	}{
		{"provider sent", func(v *ReadyReleaseControl) { v.ProviderRequest.ProviderRequestSent = true }},
		{"provider body", func(v *ReadyReleaseControl) { v.ProviderRequestJSON += " " }},
		{"provider ordinal", func(v *ReadyReleaseControl) { v.ProviderRequest.ExecutionOrdinal = 31 }},
		{"progress graph source", func(v *ReadyReleaseControl) {
			v.ProgressSnapshot.GraphID = "different-graph"
			refreshReadyDecisionTest(t, v)
		}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := cloneReadyControlTest(t, base)
			test.mutate(&value)
			assertReadyControlRejectedTest(t, &value)
		})
	}
}

func refreshReadyDecisionTest(t *testing.T, value *ReadyReleaseControl) {
	t.Helper()
	value.ProgressSnapshot = resignReadyProgressTest(t, value.ProgressSnapshot)
	decision, err := graphscheduledreconcile.Reconcile(value.ProgressSnapshot)
	if err != nil {
		t.Fatalf("Reconcile mutated progress: %v", err)
	}
	value.ReconcileDecision = decision
}

func resignReadyArtifactTest(
	t *testing.T,
	value scheduledterminal.Artifact,
) scheduledterminal.Artifact {
	t.Helper()
	value.OutputBytes = len([]byte(value.OutputText))
	value.OutputSHA256 = rawDomainDigest(readyTerminalOutputDomainTest, []byte(value.OutputText))
	encoded, err := scheduledterminal.MarshalArtifact(value)
	if err != nil {
		t.Fatalf("MarshalArtifact mutation: %v", err)
	}
	decoded, err := scheduledterminal.DecodeArtifact(encoded)
	if err != nil {
		t.Fatalf("DecodeArtifact mutation: %v", err)
	}
	return decoded
}

func assertReadyControlRejectedTest(t *testing.T, value *ReadyReleaseControl) {
	t.Helper()
	resignReadyControlTest(t, value)
	if _, err := BuildReadyAuthorization(*value); err == nil {
		t.Fatal("resigned drifted ready control was accepted")
	}
}
