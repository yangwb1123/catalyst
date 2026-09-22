package devicefabricgate

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"
)

func TestReviewFixturesAreStrictAndDoNotGrantProductionAuthority(t *testing.T) {
	proposed := readReviewFixture(t, "forge-device-fabric-activation-review-proposed-v1.json")
	request, err := DecodeReviewRequest(proposed)
	if err != nil {
		t.Fatalf("DecodeReviewRequest(proposed): %v", err)
	}
	if request.ProductionAuthorization || request.Authority != (ReviewAuthority{}) {
		t.Fatal("proposed review fixture declares production authority")
	}
	evaluation := EvaluateReview(*request)
	if evaluation.Valid || evaluation.Result.Allowed {
		t.Fatalf("proposed review unexpectedly allowed: %#v", evaluation)
	}
	if !contains(evaluation.Result.Reasons, "adr_0113_not_accepted") ||
		!contains(evaluation.Result.Reasons, "adr_0114_not_accepted") {
		t.Fatalf("proposed review lost ADR blockers: %#v", evaluation.Result.Reasons)
	}

	synthetic := readReviewFixture(t, "forge-device-fabric-activation-review-observe-synthetic-v1.json")
	syntheticRequest, err := DecodeReviewRequest(synthetic)
	if err != nil {
		t.Fatalf("DecodeReviewRequest(synthetic): %v", err)
	}
	evaluation = EvaluateReview(*syntheticRequest)
	if !evaluation.Valid || !evaluation.Result.Allowed || len(evaluation.Result.Reasons) != 0 {
		t.Fatalf("synthetic observe review = %#v", evaluation)
	}
	if syntheticRequest.ProductionAuthorization || syntheticRequest.Authority != (ReviewAuthority{}) {
		t.Fatal("synthetic review fixture declares production authority")
	}
}

func TestReviewCanonicalRoundTripAndStableIssues(t *testing.T) {
	data := readReviewFixture(t, "forge-device-fabric-activation-review-proposed-v1.json")
	request, err := DecodeReviewRequest(data)
	if err != nil {
		t.Fatalf("DecodeReviewRequest: %v", err)
	}
	canonical, err := CanonicalReviewRequestJSON(*request)
	if err != nil {
		t.Fatalf("CanonicalReviewRequestJSON: %v", err)
	}
	if !bytes.Equal(data, canonical) {
		t.Fatalf("fixture is not canonical:\n got %s\nwant %s", canonical, data)
	}
	if issues := ValidateReviewRequest(*request); len(issues) != 0 {
		t.Fatalf("valid proposed review issues = %#v", issues)
	}

	request.Authority.OpensListener = true
	request.RequestedMode = ModeExecute
	issues := ValidateReviewRequest(*request)
	want := []string{"review_authority_boundary_enabled", "review_mode_not_inventory_or_observe"}
	if !reflect.DeepEqual(issues, want) {
		t.Fatalf("mutated review issues = %#v, want %#v", issues, want)
	}
}

func TestDecodeReviewRequestRejectsAmbiguousOrNonCanonicalInput(t *testing.T) {
	valid := string(readReviewFixture(t, "forge-device-fabric-activation-review-proposed-v1.json"))
	cases := map[string]string{
		"leading whitespace": " " + valid,
		"trailing value":     valid + "null",
		"duplicate root key": strings.Replace(valid, `"request_id":"repository-adr-0114-proposed-review",`, `"request_id":"first","request_id":"repository-adr-0114-proposed-review",`, 1),
		"unknown root field": strings.Replace(valid, `{"api_version":`, `{"unknown":1,"api_version":`, 1),
		"missing authority":  strings.Replace(valid, `,"authority":{"opens_listener":false,"registers_devices":false,"persists_inventory":false,"selects_placement":false,"reserves_resources":false,"dispatches_tasks":false,"executes_remote_work":false}`, ``, 1),
		"null artifact refs": strings.Replace(valid, `"artifact_refs":["forge-core/internal/appserver"]`, `"artifact_refs":null`, 1),
	}
	for name, input := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeReviewRequest([]byte(input)); err == nil {
				t.Fatal("DecodeReviewRequest unexpectedly accepted malformed input")
			}
		})
	}
}

func TestReviewValidationRejectsEvidenceAuthorityEscalation(t *testing.T) {
	data := readReviewFixture(t, "forge-device-fabric-activation-review-observe-synthetic-v1.json")
	request, err := DecodeReviewRequest(data)
	if err != nil {
		t.Fatalf("DecodeReviewRequest: %v", err)
	}
	request.Evidence.SecurityReview.IndependentReview = false
	issues := ValidateReviewRequest(*request)
	if !contains(issues, "security_review_independent_review_required") {
		t.Fatalf("missing independent-review blocker: %#v", issues)
	}
	request.Evidence.SecurityReview.Status = "missing"
	request.Evidence.SecurityReview.ArtifactRefs = []string{"secret-token"}
	issues = ValidateReviewRequest(*request)
	if !contains(issues, "security_review_missing_claim_has_metadata") {
		t.Fatalf("missing evidence metadata blocker: %#v", issues)
	}
	request.Evidence.SecurityReview.Status = "candidate"
	request.Evidence.SecurityReview.ArtifactRefs = []string{"../secret-token"}
	issues = ValidateReviewRequest(*request)
	if !contains(issues, "security_review_artifact_ref_invalid") {
		t.Fatalf("missing path traversal blocker: %#v", issues)
	}
}

func readReviewFixture(t *testing.T, name string) []byte {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	path := filepath.Join(filepath.Dir(source), "../../../docs/contracts/fixtures", name)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return data
}
