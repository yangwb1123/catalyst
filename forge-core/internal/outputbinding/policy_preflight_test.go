package outputbinding

import (
	"bytes"
	"testing"
)

func TestRuntimePolicyBindsEveryEffectiveModeDimension(t *testing.T) {
	policy := testPolicy(t)
	base := policy.BindingSHA256
	mutators := []func(*RuntimePolicyBinding){
		func(p *RuntimePolicyBinding) { p.Mode = "balanced" },
		func(p *RuntimePolicyBinding) { p.Gates = []string{"arch", "test"} },
		func(p *RuntimePolicyBinding) { p.Reviewer = false },
		func(p *RuntimePolicyBinding) { p.EvolveDepth = "standard" },
		func(p *RuntimePolicyBinding) { p.EvolveAuthority = "propose-only" },
		func(p *RuntimePolicyBinding) { p.DiscoverDepth = "light" },
		func(p *RuntimePolicyBinding) { p.DesignDepth = "standard" },
		func(p *RuntimePolicyBinding) { p.ReviewDepth = "standard" },
		func(p *RuntimePolicyBinding) { p.ADR = false },
		func(p *RuntimePolicyBinding) { p.BuildHalt = true },
	}
	for index, mutate := range mutators {
		candidate := policy
		mutate(&candidate)
		sealed, err := SealRuntimePolicy(candidate)
		if err != nil || sealed.BindingSHA256 == base {
			t.Fatalf("effective policy dimension %d was not bound: %v", index, err)
		}
	}
}

func TestRuntimePolicySortsGatesAndRejectsDuplicates(t *testing.T) {
	policy := testPolicy(t)
	if got := policy.Gates; len(got) != 3 || got[0] != "arch" || got[2] != "test" {
		t.Fatalf("gates not sorted: %v", got)
	}
	policy.Gates = []string{"arch", "arch"}
	if _, err := SealRuntimePolicy(policy); err == nil {
		t.Fatal("duplicate policy gate was accepted")
	}
	policy = testPolicy(t)
	policy.Materiality = "HIGH"
	if _, err := SealRuntimePolicy(policy); err == nil {
		t.Fatal("unknown materiality was accepted")
	}
}

func TestPreflightBindsAllPreSpawnInputs(t *testing.T) {
	policy := testPolicy(t)
	inputs := testManifest(t)
	binding := testPreflight(t, policy, inputs, 1)
	mutators := []func(*PreflightBinding){
		func(v *PreflightBinding) { v.ArtifactInputsSHA256 = testDigest("other artifacts") },
		func(v *PreflightBinding) { v.Attempt++ },
		func(v *PreflightBinding) { v.Challenge = testDigest("other nonce") },
		func(v *PreflightBinding) { v.LocalRuntimePolicySHA256 = testDigest("other policy") },
		func(v *PreflightBinding) { v.Phase = "qa" },
		func(v *PreflightBinding) { v.PromptContextSHA256 = testDigest("other prompt") },
		func(v *PreflightBinding) { v.RunID = "other-run" },
		func(v *PreflightBinding) { v.SourceBeforeSHA256 = testDigest("other source") },
		func(v *PreflightBinding) { v.Workflow = "evolve" },
		func(v *PreflightBinding) { v.WorkflowSHA256 = testDigest("other workflow") },
	}
	for index, mutate := range mutators {
		candidate := binding
		mutate(&candidate)
		sealed, err := SealPreflight(candidate)
		if err != nil || sealed.BindingSHA256 == binding.BindingSHA256 {
			t.Fatalf("preflight input %d was not bound: %v", index, err)
		}
	}
}

func TestCanonicalPolicyAndPreflightRoundTrip(t *testing.T) {
	policy := testPolicy(t)
	policyJSON, err := CanonicalRuntimePolicyJSON(policy)
	if err != nil {
		t.Fatal(err)
	}
	decodedPolicy, err := DecodeCanonicalRuntimePolicy(policyJSON)
	if err != nil || decodedPolicy.BindingSHA256 != policy.BindingSHA256 {
		t.Fatalf("policy round trip: %v", err)
	}
	preflight := testPreflight(t, policy, testManifest(t), 1)
	preflightJSON, err := CanonicalPreflightJSON(preflight)
	if err != nil {
		t.Fatal(err)
	}
	decodedPreflight, err := DecodeCanonicalPreflight(preflightJSON)
	if err != nil || decodedPreflight.BindingSHA256 != preflight.BindingSHA256 {
		t.Fatalf("preflight round trip: %v", err)
	}
	if _, err := DecodeCanonicalPreflight(append(bytes.Clone(preflightJSON), '\n')); err == nil {
		t.Fatal("preflight accepted a trailing newline")
	}
}
