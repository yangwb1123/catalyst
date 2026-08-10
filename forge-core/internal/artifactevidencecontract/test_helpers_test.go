package artifactevidencecontract

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/artifact"
)

func validRequest() Request {
	return Request{
		APIVersion: APIVersion,
		Artifact: artifact.Record{
			Format: artifact.FormatV1, RunID: "run-20260810", Workflow: "build",
			Phase: "verify", Agent: "implementer", Model: "gpt-5.6",
			Path: "dist/报告.json", SHA256: strings.Repeat("a", 64), Size: 42,
			CreatedAt:    "2026-08-10T12:34:56.987654321+06:30",
			PromptSHA256: strings.Repeat("b", 64),
		},
		Binding: Binding{
			AggregateID: "artifact-order-42", ContextSHA256: strings.Repeat("c", 64),
			PolicySHA256: strings.Repeat("d", 64), ProjectID: "catalyst",
			Scope: "build/artifacts", Sensitivity: "internal", Sequence: 1,
			SourceRevision: "git:abcdef123", SourceTreeSHA256: strings.Repeat("e", 64),
			Subjects:            []string{"artifact:dist/report", "run:run-20260810"},
			SupersedesRecordIDs: []string{},
		},
		Canonicalization: Canonicalization,
	}
}

func canonicalValidRequest(t *testing.T) []byte {
	t.Helper()
	encoded, err := canonicalRequestJSON(validRequest())
	if err != nil {
		t.Fatalf("canonicalRequestJSON: %v", err)
	}
	return encoded
}

func assertErrorContains(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf("error = %v, want substring %q", err, want)
	}
}
