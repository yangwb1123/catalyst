package artifactevidencecontract

import (
	"fmt"
	"strings"
	"testing"
)

type requestMutation struct {
	name string
	want string
	edit func(*Request)
}

func TestTypedRequestRejectsInvalidEnvelopeAndArtifact(t *testing.T) {
	tests := []requestMutation{
		{"api version", "api_version", func(r *Request) { r.APIVersion = "v2" }},
		{"canonicalization", "canonicalization", func(r *Request) { r.Canonicalization = "json" }},
		{"empty format", "_format", func(r *Request) { r.Artifact.Format = "" }},
		{"unknown format", "_format", func(r *Request) { r.Artifact.Format = "forgeos.artifact.v2" }},
		{"empty run", "run_id", func(r *Request) { r.Artifact.RunID = "" }},
		{"empty workflow", "workflow", func(r *Request) { r.Artifact.Workflow = "" }},
		{"oversize model", "model", func(r *Request) { r.Artifact.Model = strings.Repeat("x", 4097) }},
		{"bad artifact hash", "sha256", func(r *Request) { r.Artifact.SHA256 = strings.Repeat("A", 64) }},
		{"bad prompt hash", "prompt_sha256", func(r *Request) { r.Artifact.PromptSHA256 = "bad" }},
		{"zero size", "size", func(r *Request) { r.Artifact.Size = 0 }},
		{"negative size", "size", func(r *Request) { r.Artifact.Size = -1 }},
		{"invalid time", "RFC3339Nano", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:56Q" }},
		{"pre epoch", "Unix epoch", func(r *Request) { r.Artifact.CreatedAt = "1969-12-31T23:59:59Z" }},
	}
	runMutationTests(t, tests)
}

func TestTypedRequestRejectsUnsafeArtifactPaths(t *testing.T) {
	tests := []requestMutation{
		{"empty", "path must", func(r *Request) { r.Artifact.Path = "" }},
		{"absolute", "safe normalized", func(r *Request) { r.Artifact.Path = "/etc/passwd" }},
		{"drive", "safe normalized", func(r *Request) { r.Artifact.Path = `C:\report.txt` }},
		{"backslash", "safe normalized", func(r *Request) { r.Artifact.Path = `dist\report.txt` }},
		{"empty segment", "safe normalized", func(r *Request) { r.Artifact.Path = "dist//report.txt" }},
		{"dot segment", "safe normalized", func(r *Request) { r.Artifact.Path = "dist/./report.txt" }},
		{"parent segment", "safe normalized", func(r *Request) { r.Artifact.Path = "dist/../report.txt" }},
		{"control", "forbidden Unicode", func(r *Request) { r.Artifact.Path = "dist/\nreport.txt" }},
		{"bidi", "forbidden Unicode", func(r *Request) { r.Artifact.Path = "dist/report\u202etxt" }},
	}
	runMutationTests(t, tests)
}

func TestTypedRequestRejectsInvalidBinding(t *testing.T) {
	tests := []requestMutation{
		{"aggregate", "aggregate_id", func(r *Request) { r.Binding.AggregateID = "Artifact 42" }},
		{"project", "project_id", func(r *Request) { r.Binding.ProjectID = "" }},
		{"scope", "scope", func(r *Request) { r.Binding.Scope = "Build" }},
		{"revision", "source_revision", func(r *Request) { r.Binding.SourceRevision = "git sha" }},
		{"context hash", "context_sha256", func(r *Request) { r.Binding.ContextSHA256 = "bad" }},
		{"policy hash", "policy_sha256", func(r *Request) { r.Binding.PolicySHA256 = strings.Repeat("A", 64) }},
		{"tree hash", "source_tree_sha256", func(r *Request) { r.Binding.SourceTreeSHA256 = "" }},
		{"sequence", "sequence", func(r *Request) { r.Binding.Sequence = 0 }},
		{"sensitivity", "sensitivity", func(r *Request) { r.Binding.Sensitivity = "secret" }},
		{"empty subjects", "subjects must be nonempty", func(r *Request) { r.Binding.Subjects = []string{} }},
		{"unsorted subjects", "lexicographically sorted", func(r *Request) { r.Binding.Subjects = []string{"b", "a"} }},
		{"duplicate subjects", "duplicate", func(r *Request) { r.Binding.Subjects = []string{"a", "a"} }},
		{"invalid subject", "subjects", func(r *Request) { r.Binding.Subjects = []string{"Invalid"} }},
		{"too many subjects", "exceeds", func(r *Request) { r.Binding.Subjects = repeatedIDs("s", maxItems+1) }},
		{"unsorted supersedes", "lexicographically sorted", func(r *Request) { r.Binding.SupersedesRecordIDs = []string{"b", "a"} }},
		{"duplicate supersedes", "duplicate", func(r *Request) { r.Binding.SupersedesRecordIDs = []string{"a", "a"} }},
	}
	runMutationTests(t, tests)
}

func TestTypedRequestAcceptsEveryGovernanceSensitivity(t *testing.T) {
	for _, sensitivity := range []string{"public", "internal", "confidential", "restricted"} {
		t.Run(sensitivity, func(t *testing.T) {
			request := validRequest()
			request.Binding.Sensitivity = sensitivity
			if _, err := Adapt(request); err != nil {
				t.Fatalf("Adapt: %v", err)
			}
		})
	}
}

func TestArtifactDescriptiveFieldsAcceptBoundedUnicodeText(t *testing.T) {
	request := validRequest()
	request.Artifact.Workflow = "Build Release"
	request.Artifact.Phase = "质量检查"
	request.Artifact.Agent = "Reviewer Agent"
	request.Artifact.Model = "模型 α"
	if _, err := Adapt(request); err != nil {
		t.Fatalf("Adapt: %v", err)
	}
}

func TestTypedRequestRejectsWhitespaceOnlyArtifactText(t *testing.T) {
	tests := []requestMutation{
		{"workflow", "workflow must not be blank", func(r *Request) { r.Artifact.Workflow = "   " }},
		{"phase", "phase must not be blank", func(r *Request) { r.Artifact.Phase = "\u00a0" }},
		{"agent", "agent must not be blank", func(r *Request) { r.Artifact.Agent = "\u2003" }},
		{"model", "model must not be blank", func(r *Request) { r.Artifact.Model = "  " }},
	}
	runMutationTests(t, tests)
}

func TestTypedRequestRejectsNonStrictRFC3339NanoTimestamps(t *testing.T) {
	tests := []requestMutation{
		{"offset hour overflow", "UTC offset", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:56+24:00" }},
		{"offset minute overflow", "UTC offset", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:56+00:60" }},
		{"comma fraction", "strict RFC3339Nano", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:56,1Z" }},
		{"ten fraction digits", "strict RFC3339Nano", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:56.1234567890Z" }},
	}
	runMutationTests(t, tests)
}

func runMutationTests(t *testing.T, tests []requestMutation) {
	t.Helper()
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request := validRequest()
			test.edit(&request)
			adaptation, err := Adapt(request)
			assertErrorContains(t, err, test.want)
			if adaptation != nil {
				t.Fatal("failed adaptation returned partial output")
			}
		})
	}
}

func repeatedIDs(prefix string, count int) []string {
	result := make([]string, count)
	for index := range result {
		result[index] = fmt.Sprintf("%s-%03d", prefix, index)
	}
	return result
}
