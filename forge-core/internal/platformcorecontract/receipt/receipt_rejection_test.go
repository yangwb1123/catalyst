package receipt

import (
	"bytes"
	"encoding/json"
	"os"
	"strconv"
	"strings"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/state"
)

type rejectionCorpus struct {
	APIVersion       string                    `json:"api_version"`
	EvidenceRefCases []evidenceRefCase         `json:"evidence_ref_cases"`
	ExecutorRoles    []executorRoleCase        `json:"executor_role_cases"`
	StateMachines    []stateMachineCase        `json:"state_machines"`
	TransitionCases  []transitionRejectionCase `json:"transition_cases"`
	WireCases        []wireRejectionCase       `json:"wire_cases"`
}

type executorRoleCase struct {
	ActorType string `json:"actor_type"`
}

type transitionRejectionCase struct {
	CaseID       string `json:"case_id"`
	ExpectedCode string `json:"expected_code"`
	Machine      string `json:"machine"`
	Source       string `json:"source"`
	Target       string `json:"target"`
}

type wireRejectionCase struct {
	CaseID       string         `json:"case_id"`
	ExpectedCode string         `json:"expected_code"`
	Mutations    []wireMutation `json:"mutations"`
	Target       string         `json:"target"`
}

type wireMutation struct {
	Operation string          `json:"operation"`
	Pointer   string          `json:"pointer"`
	Value     json.RawMessage `json:"value"`
}

func TestSharedReceiptRejectionCorpus(t *testing.T) {
	corpus := loadRejectionCorpus(t)
	assertRejectionCorpusCoverage(t, corpus)
	fixture := loadReceiptGolden(t)
	assertEvidenceRefCorpus(t, corpus.EvidenceRefCases, fixture.VerificationReceipt)
	assertExecutorRoleCorpus(t, corpus.ExecutorRoles, fixture.ExecutionReceipt)
	assertStateMachineCorpus(t, corpus.StateMachines)
	root := loadReceiptGoldenNode(t)
	envelope := loadEnvelopeGoldenNode(t)
	for _, field := range []string{"artifact_ref", "command_envelope", "event_envelope"} {
		root[field] = envelope[field]
	}
	for _, testCase := range corpus.WireCases {
		t.Run(testCase.CaseID, func(t *testing.T) {
			value := cloneJSONValue(t, rejectionTarget(root, testCase.Target))
			for _, mutation := range testCase.Mutations {
				applyJSONPointer(t, value, mutation)
			}
			raw, err := canonicalJSON(value, maxReceiptBytes)
			if err != nil {
				t.Fatal(err)
			}
			err = decodeRejectedWire(testCase.Target, raw, &fixture.VerificationRequest)
			if code := mustRejectionCode(t, err); string(code) != testCase.ExpectedCode {
				t.Fatalf("code = %s, want %s: %v", code, testCase.ExpectedCode, err)
			}
		})
	}
	assertTransitionCorpus(t, corpus.TransitionCases)
}

func assertExecutorRoleCorpus(t *testing.T, cases []executorRoleCase, source ExecutionReceipt) {
	t.Helper()
	expected := map[string]bool{"agent": true, "service": true, "system": true}
	if len(cases) != len(expected) {
		t.Fatalf("executor role case count = %d, want %d", len(cases), len(expected))
	}
	seen := make(map[string]bool, len(cases))
	for _, testCase := range cases {
		if !expected[testCase.ActorType] || seen[testCase.ActorType] {
			t.Fatalf("executor role cases are not exact: %v", cases)
		}
		seen[testCase.ActorType] = true
		receipt := source
		receipt.Executor.ActorRef.ActorType = core.ActorType(testCase.ActorType)
		if err := validateExecutionReceipt(&receipt); err != nil {
			t.Fatalf("executor actor_type %q rejected: %v", testCase.ActorType, err)
		}
	}
}

func assertRejectionCorpusCoverage(t *testing.T, corpus rejectionCorpus) {
	t.Helper()
	if len(corpus.WireCases) != 62 || len(corpus.TransitionCases) != 9 {
		t.Fatalf("corpus counts = %d wire + %d transition", len(corpus.WireCases), len(corpus.TransitionCases))
	}
	covered := make(map[string]bool)
	for _, testCase := range corpus.WireCases {
		covered[testCase.ExpectedCode] = true
	}
	for _, testCase := range corpus.TransitionCases {
		covered[testCase.ExpectedCode] = true
	}
	required := []string{
		"pc_document_invalid", "pc_identifier_invalid", "pc_value_invalid",
		"pc_reference_mismatch", "pc_state_invalid", "pc_transition_invalid",
		"pc_relation_mismatch",
	}
	if len(covered) != len(required) {
		t.Fatalf("corpus rejection classes = %v", covered)
	}
	for _, code := range required {
		if !covered[code] {
			t.Fatalf("corpus does not cover %s", code)
		}
	}
}

func loadRejectionCorpus(t *testing.T) rejectionCorpus {
	t.Helper()
	raw, err := os.ReadFile("../../../../docs/contracts/fixtures/platform-core-rejection-corpus-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value rejectionCorpus
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	if value.APIVersion != "forge.platform-core-rejection-corpus/v1" {
		t.Fatalf("corpus API = %q", value.APIVersion)
	}
	return value
}

func loadReceiptGoldenNode(t *testing.T) map[string]any {
	t.Helper()
	raw, err := os.ReadFile("../../../../docs/contracts/fixtures/platform-core-receipt-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	value, err := parseStrictJSON(raw, maxReceiptBytes)
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}

func loadEnvelopeGoldenNode(t *testing.T) map[string]any {
	t.Helper()
	raw, err := os.ReadFile("../../../../docs/contracts/fixtures/platform-core-envelope-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	value, err := parseStrictJSON(raw, maxReceiptBytes)
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}

func rejectionTarget(root map[string]any, target string) any {
	if target == "verification_exchange" {
		return root["verification_receipt"]
	}
	return root[target]
}

func cloneJSONValue(t *testing.T, value any) any {
	t.Helper()
	raw, err := canonicalJSON(value, maxReceiptBytes)
	if err != nil {
		t.Fatal(err)
	}
	clone, err := parseStrictJSON(raw, maxReceiptBytes)
	if err != nil {
		t.Fatal(err)
	}
	return clone
}

func applyJSONPointer(t *testing.T, root any, mutation wireMutation) {
	t.Helper()
	replacement, err := parseStrictJSON(mutation.Value, maxReceiptBytes)
	if err != nil {
		t.Fatal(err)
	}
	parts := strings.Split(strings.TrimPrefix(mutation.Pointer, "/"), "/")
	current := root
	for _, part := range parts[:len(parts)-1] {
		current = jsonChild(t, current, part)
	}
	last := parts[len(parts)-1]
	switch parent := current.(type) {
	case map[string]any:
		switch mutation.Operation {
		case "replace":
			parent[last] = replacement
		case "remove":
			if _, exists := parent[last]; !exists {
				t.Fatalf("JSON pointer field %q missing", last)
			}
			delete(parent, last)
		default:
			t.Fatalf("unsupported mutation operation %q", mutation.Operation)
		}
	case []any:
		if mutation.Operation != "replace" {
			t.Fatalf("operation %q is unsupported for arrays", mutation.Operation)
		}
		index, err := strconv.Atoi(last)
		if err != nil || index < 0 || index >= len(parent) {
			t.Fatalf("invalid JSON pointer %q", mutation.Pointer)
		}
		parent[index] = replacement
	default:
		t.Fatalf("JSON pointer %q reached %T", mutation.Pointer, current)
	}
}

func jsonChild(t *testing.T, value any, part string) any {
	t.Helper()
	switch parent := value.(type) {
	case map[string]any:
		child, ok := parent[part]
		if !ok {
			t.Fatalf("JSON pointer field %q missing", part)
		}
		return child
	case []any:
		index, err := strconv.Atoi(part)
		if err != nil || index < 0 || index >= len(parent) {
			t.Fatalf("JSON pointer index %q invalid", part)
		}
		return parent[index]
	default:
		t.Fatalf("JSON pointer cannot traverse %T", value)
	}
	return nil
}

func decodeRejectedWire(target string, raw []byte, request *VerificationRequest) error {
	switch target {
	case "execution_receipt":
		_, err := DecodeCanonicalExecutionReceipt(raw)
		return err
	case "verification_request":
		_, err := DecodeCanonicalVerificationRequest(raw)
		return err
	case "verification_receipt", "verification_exchange":
		receipt, err := DecodeCanonicalVerificationReceipt(raw)
		if err != nil || target == "verification_receipt" {
			return err
		}
		return ValidateVerificationExchange(request, receipt)
	case "event_envelope":
		_, err := core.DecodeCanonicalEventEnvelope(raw)
		return err
	case "artifact_ref":
		_, err := core.DecodeCanonicalArtifactRef(raw)
		return err
	case "command_envelope":
		_, err := core.DecodeCanonicalCommandEnvelope(raw)
		return err
	default:
		return rejectf(rejectionValueInvalid, "unknown corpus target %q", target)
	}
}

func assertTransitionCorpus(t *testing.T, cases []transitionRejectionCase) {
	t.Helper()
	for _, testCase := range cases {
		t.Run(testCase.CaseID, func(t *testing.T) {
			err := stateTransition(testCase.Machine, testCase.Source, testCase.Target)
			if code := mustRejectionCode(t, err); string(code) != testCase.ExpectedCode {
				t.Fatalf("code = %s, want %s", code, testCase.ExpectedCode)
			}
		})
	}
}

func TestDeclaredStateEdgesAcceptOnlyExplicitTransitions(t *testing.T) {
	for _, operation := range []func() error{
		func() error {
			return state.ValidateWorkItemTransition("verifying", "completed")
		},
		func() error { return state.ValidateAttemptTransition("running", "completed") },
		func() error { return state.ValidateActionTransition("started", "finished") },
	} {
		if err := operation(); err != nil {
			t.Fatal(err)
		}
	}
	if code := mustRejectionCode(t, state.ValidateAttemptTransition("unknown", "running")); code != rejectionStateInvalid {
		t.Fatalf("unknown state code = %s", code)
	}
}

func TestEveryPublicReceiptOperationReturnsCodedErrors(t *testing.T) {
	operations := map[string]func() error{
		"canonical execution": func() error {
			_, err := CanonicalExecutionReceiptJSON(nil)
			return err
		},
		"decode execution": func() error {
			_, err := DecodeCanonicalExecutionReceipt([]byte("{}"))
			return err
		},
		"digest execution": func() error { _, err := ExecutionReceiptSHA256(nil); return err },
		"canonical request": func() error {
			_, err := CanonicalVerificationRequestJSON(nil)
			return err
		},
		"decode request": func() error {
			_, err := DecodeCanonicalVerificationRequest([]byte("{}"))
			return err
		},
		"digest request": func() error { _, err := VerificationRequestSHA256(nil); return err },
		"canonical verification": func() error {
			_, err := CanonicalVerificationReceiptJSON(nil)
			return err
		},
		"decode verification": func() error {
			_, err := DecodeCanonicalVerificationReceipt([]byte("{}"))
			return err
		},
		"digest verification": func() error { _, err := VerificationReceiptSHA256(nil); return err },
		"exchange":            func() error { return ValidateVerificationExchange(nil, nil) },
	}
	for name, operation := range operations {
		t.Run(name, func(t *testing.T) {
			if _, ok := core.RejectionCodeOf(operation()); !ok {
				t.Fatal("public operation returned an uncoded error")
			}
		})
	}
}

func TestProgrammaticExecutionValidationUsesStagedRejections(t *testing.T) {
	fixture := loadReceiptGolden(t)
	value := fixture.ExecutionReceipt
	value.TerminalState = "running"
	value.AttemptRef.EntityID = "atm_0000000000000000000000000m"
	if code := mustRejectionCode(t, validateExecutionReceipt(&value)); code != rejectionReferenceMismatch {
		t.Fatalf("state/reference code = %s", code)
	}
	value = fixture.ExecutionReceipt
	value.ReasonCodes = nil
	value.AttemptRef.EntityID = "atm_0000000000000000000000000m"
	if code := mustRejectionCode(t, validateExecutionReceipt(&value)); code != rejectionDocumentInvalid {
		t.Fatalf("shape/reference code = %s", code)
	}
}
