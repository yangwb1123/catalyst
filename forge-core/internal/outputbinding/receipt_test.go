package outputbinding

import (
	"bytes"
	"errors"
	"io"
	"strings"
	"testing"
)

func TestReceiptRoundTripsAndNeverStoresOutputContent(t *testing.T) {
	receipt := testReceipt(t, 1, nil)
	encoded, err := CanonicalReceiptJSON(receipt)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(encoded, []byte("SUPER_SECRET_RAW_OUTPUT")) ||
		bytes.Contains(encoded, []byte(`"raw_output":`)) {
		t.Fatal("receipt retained raw output content")
	}
	decoded, err := DecodeCanonicalReceipt(encoded)
	if err != nil || decoded.ReceiptSHA256 != receipt.ReceiptSHA256 {
		t.Fatalf("receipt round trip: %v", err)
	}
	if receipt.ReceiptSHA256 != "6b4b8ead82ee3ce5e13122cafb4dc70df2f0115bc597694c14a1009ff6a817b6" {
		t.Fatalf("receipt digest = %s", receipt.ReceiptSHA256)
	}
}

func TestReceiptRejectsNestedAndIdentityMismatchesBeforeSeal(t *testing.T) {
	base := testReceipt(t, 1, nil)
	mutators := []func(*AgentOutputReceipt){
		func(r *AgentOutputReceipt) { r.Agent = "qa" },
		func(r *AgentOutputReceipt) { r.Phase = "qa" },
		func(r *AgentOutputReceipt) { r.Model = "sonnet" },
		func(r *AgentOutputReceipt) { r.Executor = "/other" },
		func(r *AgentOutputReceipt) { r.ArtifactInputsSHA256 = testDigest("wrong") },
		func(r *AgentOutputReceipt) { r.ArtifactOutputsSHA256 = testDigest("wrong") },
		func(r *AgentOutputReceipt) { r.LocalRuntimePolicySHA256 = testDigest("wrong") },
		func(r *AgentOutputReceipt) { r.BindingSHA256 = testDigest("wrong") },
		func(r *AgentOutputReceipt) { r.PromptContextSHA256 = testDigest("wrong") },
		func(r *AgentOutputReceipt) { r.SourceBeforeSHA256 = testDigest("wrong") },
	}
	for index, mutate := range mutators {
		candidate := base
		mutate(&candidate)
		if _, err := SealReceipt(candidate); err == nil {
			t.Fatalf("receipt mismatch %d was accepted", index)
		}
	}
}

func TestReceiptGenesisAndVerdictAreStrict(t *testing.T) {
	prior := testDigest("prior")
	if _, err := SealReceipt(withPrior(testReceipt(t, 1, nil), &prior)); err == nil {
		t.Fatal("genesis prior link was accepted")
	}
	base := testReceipt(t, 1, nil)
	base.LedgerSequence = 2
	base.PriorReceiptSHA256 = nil
	if _, err := SealReceipt(base); err == nil {
		t.Fatal("non-genesis null prior link was accepted")
	}
	base = testReceipt(t, 1, nil)
	badVerdict := "approve"
	base.Verdict = &badVerdict
	if _, err := SealReceipt(base); err == nil {
		t.Fatal("non-canonical verdict was accepted")
	}
	base = testReceipt(t, 1, nil)
	base.Verdict = nil
	if _, err := SealReceipt(base); err != nil {
		t.Fatalf("nullable verdict was rejected: %v", err)
	}
}

func withPrior(receipt AgentOutputReceipt, prior *string) AgentOutputReceipt {
	receipt.PriorReceiptSHA256 = prior
	return receipt
}

func TestReceiptDecodeRejectsAdversarialWireForms(t *testing.T) {
	encoded, err := CanonicalReceiptJSON(testReceipt(t, 1, nil))
	if err != nil {
		t.Fatal(err)
	}
	mutations := receiptWireMutations(encoded)
	for index, mutation := range mutations {
		if _, err := DecodeCanonicalReceipt(mutation); err == nil {
			t.Fatalf("receipt wire mutation %d was accepted", index)
		}
	}
}

func receiptWireMutations(encoded []byte) [][]byte {
	raw := string(encoded)
	duplicate := strings.Replace(raw, `{"agent":`, `{"agent":"reviewer","agent":`, 1)
	unknown := strings.Replace(raw, `"api_version":`, `"unknown":0,"api_version":`, 1)
	nullPolicy := strings.Replace(raw, `"runtime_policy":{`, `"runtime_policy":null,"discarded":{`, 1)
	reordered := strings.Replace(raw,
		`{"agent":"reviewer","api_version":"forgeos.agent-output-receipt/v1",`,
		`{"api_version":"forgeos.agent-output-receipt/v1","agent":"reviewer",`, 1)
	return [][]byte{
		append(bytes.Clone(encoded), '\n'), append([]byte(" "), encoded...),
		append(bytes.Clone(encoded), []byte(`{}`)...), []byte(duplicate), []byte(unknown),
		[]byte(nullPolicy), []byte(reordered), append(bytes.Clone(encoded), 0xff),
	}
}

func TestReceiptChainRequiresContiguousExactPriorDigest(t *testing.T) {
	first := testReceipt(t, 1, nil)
	second := receiptForAttempt(t, 2, &first.ReceiptSHA256, 2, testDigest("challenge-2"))
	if err := ValidateReceiptChain([]AgentOutputReceipt{first, second}); err != nil {
		t.Fatal(err)
	}
	wrong := testDigest("wrong prior")
	broken := testReceipt(t, 2, &wrong)
	if err := ValidateReceiptChain([]AgentOutputReceipt{first, broken}); err == nil {
		t.Fatal("broken receipt prior link was accepted")
	}
	if err := ValidateReceiptChain([]AgentOutputReceipt{second}); err == nil {
		t.Fatal("non-genesis first journal item was accepted")
	}
}

func TestReceiptChainRejectsAttemptAndNonceReplay(t *testing.T) {
	first := testReceipt(t, 1, nil)
	tests := []struct {
		name      string
		attempt   int64
		challenge string
	}{
		{name: "attempt", attempt: 1, challenge: testDigest("new-challenge")},
		{name: "challenge", attempt: 2, challenge: first.Challenge},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			second := receiptForAttempt(t, 2, &first.ReceiptSHA256, test.attempt, test.challenge)
			if err := ValidateReceiptChain([]AgentOutputReceipt{first, second}); err == nil {
				t.Fatal("receipt replay was accepted")
			}
		})
	}
	state := newReceiptChainState()
	state.bindings[first.BindingSHA256] = struct{}{}
	if err := state.accept(first); err == nil || !strings.Contains(err.Error(), "binding") {
		t.Fatalf("duplicate binding error = %v", err)
	}
}

func receiptForAttempt(t *testing.T, sequence int64, prior *string,
	attempt int64, challenge string) AgentOutputReceipt {
	t.Helper()
	receipt := testReceipt(t, sequence, prior)
	receipt.Attempt, receipt.Challenge = attempt, challenge
	preflight, err := SealPreflight(PreflightBinding{
		ArtifactInputsSHA256: receipt.ArtifactInputsSHA256, Attempt: attempt,
		Challenge: challenge, LocalRuntimePolicySHA256: receipt.LocalRuntimePolicySHA256,
		Phase: receipt.Phase, PromptContextSHA256: receipt.PromptContextSHA256,
		RunID: receipt.RunID, SourceBeforeSHA256: receipt.SourceBeforeSHA256,
		Workflow: receipt.Workflow, WorkflowSHA256: receipt.RuntimePolicy.WorkflowSHA256,
	})
	if err != nil {
		t.Fatal(err)
	}
	receipt.BindingSHA256 = preflight.BindingSHA256
	sealed, err := SealReceipt(receipt)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func TestAppendValidatedWritesOneLineButClaimsNoDurability(t *testing.T) {
	receipt := testReceipt(t, 1, nil)
	var output bytes.Buffer
	if err := AppendValidated(&output, receipt); err != nil {
		t.Fatal(err)
	}
	canonical, _ := CanonicalReceiptJSON(receipt)
	if !bytes.Equal(output.Bytes(), append(canonical, '\n')) {
		t.Fatal("append output was not one canonical JSONL line")
	}
	if err := AppendValidated(shortWriter{}, receipt); !errors.Is(err, io.ErrShortWrite) {
		t.Fatalf("short write error = %v", err)
	}
	if err := AppendValidated(nil, receipt); err == nil {
		t.Fatal("nil writer was accepted")
	}
}

type shortWriter struct{}

func (shortWriter) Write(value []byte) (int, error) { return len(value) - 1, nil }

func TestSealReceiptDetachesNestedSlicesAndPointers(t *testing.T) {
	receipt := testReceipt(t, 2, stringRef(testDigest("prior")))
	sealed, err := SealReceipt(receipt)
	if err != nil {
		t.Fatal(err)
	}
	*receipt.PriorReceiptSHA256 = testDigest("changed")
	*receipt.Verdict = "REQUEST_CHANGES"
	receipt.RuntimePolicy.Gates[0] = "changed"
	receipt.ArtifactInputs.Items[0].Path = "changed"
	if *sealed.PriorReceiptSHA256 == *receipt.PriorReceiptSHA256 ||
		*sealed.Verdict == *receipt.Verdict || sealed.RuntimePolicy.Gates[0] == "changed" ||
		sealed.ArtifactInputs.Items[0].Path == "changed" {
		t.Fatal("sealed receipt aliases caller-owned memory")
	}
}

func TestReceiptPreservesEmptyManifestsAndBindsEmptyOutput(t *testing.T) {
	receipt := testReceipt(t, 1, nil)
	receipt.ArtifactInputs = testManifest(t)
	receipt.ArtifactInputsSHA256 = receipt.ArtifactInputs.ManifestSHA256
	receipt.ArtifactOutputs = testManifest(t)
	receipt.ArtifactOutputsSHA256 = receipt.ArtifactOutputs.ManifestSHA256
	receipt.RawOutputBytes, receipt.RawOutputSHA256 = 0, SHA256(nil)
	receipt.SemanticOutputBytes, receipt.SemanticOutputSHA256 = 0, SHA256(nil)
	preflight := testPreflight(t, receipt.RuntimePolicy, receipt.ArtifactInputs, receipt.Attempt)
	receipt.BindingSHA256 = preflight.BindingSHA256
	sealed, err := SealReceipt(receipt)
	if err != nil {
		t.Fatal(err)
	}
	if sealed.ArtifactInputs.Items == nil || sealed.ArtifactOutputs.Items == nil {
		t.Fatal("receipt clone changed empty artifact arrays to null")
	}
	encoded, err := CanonicalReceiptJSON(sealed)
	if err != nil || bytes.Count(encoded, []byte(`"items":[]`)) != 2 {
		t.Fatalf("empty receipt artifacts are not canonical arrays: %v, %s", err, encoded)
	}
}

func TestReceiptRejectsFalseEmptyOutputHashAndUnprefixedRevision(t *testing.T) {
	receipt := testReceipt(t, 1, nil)
	receipt.RawOutputBytes = 0
	if _, err := SealReceipt(receipt); err == nil {
		t.Fatal("zero raw bytes with a non-empty digest were accepted")
	}
	receipt = testReceipt(t, 1, nil)
	receipt.SemanticOutputBytes = 0
	if _, err := SealReceipt(receipt); err == nil {
		t.Fatal("zero semantic bytes with a non-empty digest were accepted")
	}
	receipt = testReceipt(t, 1, nil)
	receipt.SourceRevision = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	if _, err := SealReceipt(receipt); err == nil {
		t.Fatal("unprefixed source revision was accepted")
	}
	receipt = testReceipt(t, 1, nil)
	receipt.ObservedAtUnixMS = maxSequence + 1
	if _, err := SealReceipt(receipt); err == nil {
		t.Fatal("timestamp outside canonical integer range was accepted")
	}
}

func TestReceiptVerdictRequiresReviewerV2Policy(t *testing.T) {
	for _, mutate := range []func(*RuntimePolicyBinding){
		func(policy *RuntimePolicyBinding) { policy.Reviewer = false },
		func(policy *RuntimePolicyBinding) { policy.VerdictContract = "reviewer_v1" },
	} {
		receipt := testReceipt(t, 1, nil)
		policy := receipt.RuntimePolicy
		mutate(&policy)
		sealedPolicy, err := SealRuntimePolicy(policy)
		if err != nil {
			t.Fatal(err)
		}
		receipt.RuntimePolicy = sealedPolicy
		receipt.LocalRuntimePolicySHA256 = sealedPolicy.BindingSHA256
		preflight, err := SealPreflight(PreflightBinding{
			ArtifactInputsSHA256: receipt.ArtifactInputsSHA256, Attempt: receipt.Attempt,
			Challenge: receipt.Challenge, LocalRuntimePolicySHA256: sealedPolicy.BindingSHA256,
			Phase: receipt.Phase, PromptContextSHA256: receipt.PromptContextSHA256,
			RunID: receipt.RunID, SourceBeforeSHA256: receipt.SourceBeforeSHA256,
			Workflow: receipt.Workflow, WorkflowSHA256: sealedPolicy.WorkflowSHA256,
		})
		if err != nil {
			t.Fatal(err)
		}
		receipt.BindingSHA256 = preflight.BindingSHA256
		if _, err := SealReceipt(receipt); err == nil {
			t.Fatal("control verdict without reviewer_v2 policy was accepted")
		}
	}
}
