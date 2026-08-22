package outputbinding

import "testing"

func testDigest(label string) string { return SHA256([]byte(label)) }

func stringRef(value string) *string { return &value }

func testManifest(t *testing.T, items ...ManifestItem) ArtifactManifest {
	t.Helper()
	manifest, err := SealManifest(items)
	if err != nil {
		t.Fatal(err)
	}
	return manifest
}

func testPolicy(t *testing.T) RuntimePolicyBinding {
	t.Helper()
	policy, err := SealRuntimePolicy(RuntimePolicyBinding{
		ADR: true, Agent: "reviewer", BuildHalt: false, DesignDepth: "full",
		DiscoverDepth: "full", Effect: "verify", EvolveAuthority: "auto-act",
		EvolveDepth: "thorough", Executor: "/usr/bin/claude", FreshContext: true,
		Gates: []string{"test", "arch", "build"}, Lifecycle: "production",
		Materiality: "L4", Mode: "engineering", Model: "opus",
		OutputBindingContract: localProfile, Phase: "reviewer", Readonly: true,
		ReviewDepth: "full", Reviewer: true, Stage: "build",
		VerdictContract: "reviewer_v2", WorkflowSHA256: testDigest("workflow"),
	})
	if err != nil {
		t.Fatal(err)
	}
	return policy
}

func testPreflight(t *testing.T, policy RuntimePolicyBinding,
	inputs ArtifactManifest, attempt int64) PreflightBinding {
	t.Helper()
	binding, err := SealPreflight(PreflightBinding{
		ArtifactInputsSHA256: inputs.ManifestSHA256, Attempt: attempt,
		Challenge: testDigest("challenge"), LocalRuntimePolicySHA256: policy.BindingSHA256,
		Phase: policy.Phase, PromptContextSHA256: testDigest("prebinding prompt"),
		RunID: "run-0064", SourceBeforeSHA256: testDigest("source before"),
		Workflow: "build", WorkflowSHA256: policy.WorkflowSHA256,
	})
	if err != nil {
		t.Fatal(err)
	}
	return binding
}

func testReceipt(t *testing.T, sequence int64, prior *string) AgentOutputReceipt {
	t.Helper()
	inputs := testManifest(t, ManifestItem{Bytes: 7, Path: "docs/input.md", SHA256: testDigest("input")})
	outputs := testManifest(t, ManifestItem{Bytes: 8, Path: "docs/output.md", SHA256: testDigest("output")})
	policy := testPolicy(t)
	preflight := testPreflight(t, policy, inputs, 1)
	verdict := "APPROVE"
	receipt, err := SealReceipt(AgentOutputReceipt{
		Agent: policy.Agent, ArtifactInputs: inputs, ArtifactInputsSHA256: inputs.ManifestSHA256,
		ArtifactOutputs: outputs, ArtifactOutputsSHA256: outputs.ManifestSHA256,
		Attempt: preflight.Attempt, BindingSHA256: preflight.BindingSHA256,
		Challenge: preflight.Challenge, Executor: policy.Executor,
		FinalPromptSHA256: testDigest("final prompt"), LedgerSequence: sequence,
		LocalRuntimePolicySHA256: policy.BindingSHA256, Model: policy.Model,
		ObservedAtUnixMS: 1_786_500_000_123, Phase: policy.Phase,
		PriorReceiptSHA256: prior, PromptContextSHA256: preflight.PromptContextSHA256,
		RawOutputBytes: 23, RawOutputSHA256: testDigest("SUPER_SECRET_RAW_OUTPUT"),
		RunID: preflight.RunID, RuntimePolicy: policy,
		SemanticOutputBytes: 7, SemanticOutputSHA256: testDigest("APPROVE"),
		SourceAfterSHA256: testDigest("source after"), SourceBeforeSHA256: preflight.SourceBeforeSHA256,
		SourceRevision: "git-sha1:" + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		Verdict:        &verdict, Workflow: preflight.Workflow,
	})
	if err != nil {
		t.Fatal(err)
	}
	return receipt
}
