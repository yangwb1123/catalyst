package artifactevidencecontract

import (
	"strings"
	"testing"
)

func TestEveryArtifactProvenanceFieldChangesSourceRequestAndOutputIdentity(t *testing.T) {
	tests := []requestMutation{
		{"run", "", func(r *Request) { r.Artifact.RunID = "run-20260811" }},
		{"workflow", "", func(r *Request) { r.Artifact.Workflow = "release" }},
		{"phase", "", func(r *Request) { r.Artifact.Phase = "package" }},
		{"agent", "", func(r *Request) { r.Artifact.Agent = "reviewer" }},
		{"model", "", func(r *Request) { r.Artifact.Model = "gpt-5.7" }},
		{"path", "", func(r *Request) { r.Artifact.Path = "dist/other.json" }},
		{"content hash", "", func(r *Request) { r.Artifact.SHA256 = strings.Repeat("f", 64) }},
		{"size", "", func(r *Request) { r.Artifact.Size = 43 }},
		{"time", "", func(r *Request) { r.Artifact.CreatedAt = "2026-08-10T12:34:57+06:30" }},
		{"prompt hash", "", func(r *Request) { r.Artifact.PromptSHA256 = strings.Repeat("0", 64) }},
	}
	base := mustAdapt(t, validRequest())
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			variantRequest := validRequest()
			test.edit(&variantRequest)
			variant := mustAdapt(t, variantRequest)
			if variant.SourceSHA256 == base.SourceSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatalf("artifact mutation did not alter all identities: %#v", variant)
			}
		})
	}
}

func TestEveryBindingFieldChangesRequestAndOutputButNotSourceIdentity(t *testing.T) {
	tests := bindingIdentityMutations()
	base := mustAdapt(t, validRequest())
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			variantRequest := validRequest()
			test.edit(&variantRequest)
			variant := mustAdapt(t, variantRequest)
			if variant.SourceSHA256 != base.SourceSHA256 {
				t.Fatal("binding mutation changed source identity")
			}
			if variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("binding mutation did not change request and output identities")
			}
		})
	}
}

func bindingIdentityMutations() []requestMutation {
	return []requestMutation{
		{"aggregate", "", func(r *Request) { r.Binding.AggregateID = "artifact-order-43" }},
		{"context", "", func(r *Request) { r.Binding.ContextSHA256 = strings.Repeat("0", 64) }},
		{"policy", "", func(r *Request) { r.Binding.PolicySHA256 = strings.Repeat("1", 64) }},
		{"project", "", func(r *Request) { r.Binding.ProjectID = "catalyst-next" }},
		{"scope", "", func(r *Request) { r.Binding.Scope = "release/artifacts" }},
		{"sequence", "", func(r *Request) { r.Binding.Sequence = 2 }},
		{"sensitivity", "", func(r *Request) { r.Binding.Sensitivity = "restricted" }},
		{"revision", "", func(r *Request) { r.Binding.SourceRevision = "git:abcdef124" }},
		{"tree", "", func(r *Request) { r.Binding.SourceTreeSHA256 = strings.Repeat("2", 64) }},
		{"subjects", "", func(r *Request) { r.Binding.Subjects = []string{"artifact:dist/other", "run:run-20260810"} }},
		{"supersedes", "", func(r *Request) { r.Binding.SupersedesRecordIDs = []string{"artifact-evidence-prior"} }},
	}
}

func mustAdapt(t *testing.T, request Request) *Adaptation {
	t.Helper()
	adaptation, err := Adapt(request)
	if err != nil {
		t.Fatalf("Adapt: %v", err)
	}
	return adaptation
}
