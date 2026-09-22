package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

// TestLocalRunnerPreviewContractFixture consumes the same metadata-only
// observation mirrored by the ecosystem receivers. The fixture is supplied by
// scripts/test-forge-contracts.sh so the Core package remains usable without a
// repository-relative filesystem assumption.
func TestLocalRunnerPreviewContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_LOCAL_RUNNER_PREVIEW_FIXTURE")
	if path == "" {
		t.Skip("FORGE_LOCAL_RUNNER_PREVIEW_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := rejectDuplicateFields(encoded); err != nil {
		t.Fatalf("local Runner preview fixture contains duplicate fields: %v", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture LocalRunnerPreviewObservation
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode local Runner preview fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("local Runner preview fixture has trailing JSON: %v", err)
	}
	if err := fixture.Validate(); err != nil {
		t.Fatalf("invalid local Runner preview fixture: %v", err)
	}
	if fixture.SchemaVersion != LocalRunnerPreviewSchemaVersion ||
		fixture.EvaluationMode != LocalRunnerPreviewEvaluationMode ||
		fixture.Intent.Owner.Subject != "user-1" ||
		fixture.Intent.ConversationID != "conversation-001" ||
		fixture.Intent.RunID != "run-001" ||
		fixture.Intent.TargetID != "runner-1" ||
		fixture.DispositionKind != "completed" ||
		fixture.ObservedAtMS != 300 || fixture.OutputBytes != 8 || fixture.ExitCode != 0 ||
		fixture.SessionReceipt.SelectedTargetID != nil ||
		fixture.Intent.SelectedTargetID != nil ||
		fixture.Authority != (LocalRunnerPreviewAuthority{}) {
		t.Fatalf("local Runner preview fixture is not metadata-only: %#v", fixture)
	}
}

func TestLocalRunnerPreviewContractFixtureRejectsUnknownDuplicateAndAuthorityMutation(t *testing.T) {
	path := os.Getenv("FORGE_LOCAL_RUNNER_PREVIEW_FIXTURE")
	if path == "" {
		t.Skip("FORGE_LOCAL_RUNNER_PREVIEW_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutations := []struct {
		name string
		data []byte
	}{
		{name: "unknown", data: append(append([]byte{}, bytes.TrimSpace(encoded)...), []byte(`{"unexpected":true}`)...)},
		{name: "duplicate", data: bytes.Replace(encoded,
			[]byte(`"audit_published": false
  }
}`),
			[]byte(`"audit_published": false,
    "audit_published": false
  }
}`), 1)},
		{name: "authority", data: bytes.Replace(encoded, []byte(`"execution_authorized": false`), []byte(`"execution_authorized": true`), 1)},
		{name: "selected target", data: bytes.Replace(encoded, []byte(`"selected_target_id": null`), []byte(`"selected_target_id": "runner-1"`), 1)},
	}
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			if mutation.name == "unknown" {
				// The appended object is trailing JSON, which is intentionally
				// rejected by the same strict framing check.
				if _, err := decodeLocalRunnerPreviewFixture(mutation.data); err == nil {
					t.Fatal("unknown or trailing local Runner preview field accepted")
				}
				return
			}
			if _, err := decodeLocalRunnerPreviewFixture(mutation.data); err == nil {
				t.Fatalf("invalid %s mutation accepted", mutation.name)
			}
		})
	}
}

func decodeLocalRunnerPreviewFixture(encoded []byte) (LocalRunnerPreviewObservation, error) {
	if err := rejectDuplicateFields(encoded); err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var fixture LocalRunnerPreviewObservation
	if err := decoder.Decode(&fixture); err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return LocalRunnerPreviewObservation{}, errInvalidRequest
		}
		return LocalRunnerPreviewObservation{}, err
	}
	if err := fixture.Validate(); err != nil {
		return LocalRunnerPreviewObservation{}, err
	}
	return fixture, nil
}
