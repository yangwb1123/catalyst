package evolverepolocatorevidencecontract

import (
	"strings"
	"testing"
)

func TestTypedRequestRejectsInvalidEnvelopeBindingAndObservation(t *testing.T) {
	tests := []requestMutation{
		{"api version", "api_version", func(r *Request) { r.APIVersion = "v2" }},
		{"canonicalization", "canonicalization", func(r *Request) { r.Canonicalization = "json" }},
		{"observation version", "api_version", func(r *Request) { r.Observation.APIVersion = "v2" }},
		{"observation canonicalization", "canonicalization", func(r *Request) { r.Observation.Canonicalization = "json" }},
		{"zero content bytes", "bytes", func(r *Request) { r.Observation.Content.Bytes = 0 }},
		{"large content", "bytes", func(r *Request) { r.Observation.Content.Bytes = maxContentBytes + 1 }},
		{"content hash", "sha256", func(r *Request) { r.Observation.Content.SHA256 = strings.Repeat("A", 64) }},
		{"blank detail", "detail", func(r *Request) { r.Observation.Locator.Detail = " \t" }},
		{"control detail", "detail", func(r *Request) { r.Observation.Locator.Detail = "bad\u0085detail" }},
		{"large detail", "detail", func(r *Request) { r.Observation.Locator.Detail = strings.Repeat("é", maxDetailBytes/2+1) }},
		{"negative line", "line", func(r *Request) { r.Observation.Locator.Line = -1 }},
		{"negative time", "observed_at", func(r *Request) { r.Observation.ObservedAtUnixMS = -1 }},
		{"producer parameters", "parameters_sha256", func(r *Request) { r.Observation.Producer.ParametersSHA256 = "bad" }},
		{"producer id", "producer_id", func(r *Request) { r.Observation.Producer.ProducerID = "Forge Scanner" }},
		{"producer type", "producer_type", func(r *Request) { r.Observation.Producer.ProducerType = "human" }},
		{"scan contract", "contract", func(r *Request) { r.Observation.ScanContext.Contract = "v2" }},
		{"scan depth", "depth", func(r *Request) { r.Observation.ScanContext.Depth = "deep" }},
		{"scan dimension", "dimension", func(r *Request) { r.Observation.ScanContext.Dimension = "style" }},
		{"scan relation", "relation", func(r *Request) { r.Observation.ScanContext.Relation = "unknown" }},
		{"report hash", "report_sha256", func(r *Request) { r.Observation.ScanContext.ReportSHA256 = "bad" }},
		{"source revision", "source_revision", func(r *Request) { r.Observation.Source.SourceRevision = "Git SHA" }},
		{"source hash", "source_tree_sha256", func(r *Request) { r.Observation.Source.SourceTreeSHA256 = "bad" }},
		{"sequence", "sequence", func(r *Request) { r.Binding.Sequence = 0 }},
		{"sensitivity", "sensitivity", func(r *Request) { r.Binding.Sensitivity = "secret" }},
		{"empty subjects", "subjects", func(r *Request) { r.Binding.Subjects = []string{} }},
		{"unsorted subjects", "sorted", func(r *Request) { r.Binding.Subjects = []string{"z", "a"} }},
		{"duplicate subjects", "duplicate", func(r *Request) { r.Binding.Subjects = []string{"a", "a"} }},
		{"too many supersedes", "exceeds", func(r *Request) { r.Binding.SupersedesRecordIDs = repeatedIDs("r", maxItems+1) }},
	}
	runMutationTests(t, tests)
}

func TestTypedRequestRejectsUnsafeRepositoryPaths(t *testing.T) {
	unsafe := []string{
		"", "   ", "/tmp/file", `C:\tmp\file`, `a\b`, "a//b", "a/./b", "a/../b", "a/", "./a",
		".git/config", ".GIT/config", ".Git/config", ".forge/state", ".FORGE/state", ".Forge/state",
		strings.Repeat("a", maxPathScalars+1), "dir/bad\u0085path",
	}
	for _, value := range unsafe {
		t.Run(value, func(t *testing.T) {
			request := validRequest()
			request.Observation.Locator.Path = value
			adaptation, err := Adapt(request)
			if err == nil || adaptation != nil {
				t.Fatalf("unsafe path %q accepted: %#v, %v", value, adaptation, err)
			}
		})
	}
	for _, value := range []string{"README.md", ".github/workflows/ci.yml", "目录/子目录/规则.yaml", "space dir/file.txt", "nested/.git/config"} {
		t.Run("valid-"+value, func(t *testing.T) {
			request := validRequest()
			request.Observation.Locator.Path = value
			if _, err := Adapt(request); err != nil {
				t.Fatalf("valid path %q: %v", value, err)
			}
		})
	}
}

func TestOpportunityRelationMatrix(t *testing.T) {
	missing := validRequest()
	missing.Observation.ScanContext.OpportunityID = nil
	_, err := Adapt(missing)
	assertErrorContains(t, err, "opportunity_id")
	for _, value := range []string{"x:y", "x/y", strings.Repeat("a", 65)} {
		request := validRequest()
		request.Observation.ScanContext.OpportunityID = &value
		_, err := Adapt(request)
		assertErrorContains(t, err, "Evolve identifier")
	}

	for _, relation := range []string{"clear", "finding"} {
		t.Run(relation+"-requires-null", func(t *testing.T) {
			request := validRequest()
			request.Observation.ScanContext.Relation = relation
			_, err := Adapt(request)
			assertErrorContains(t, err, "must be null")
		})
		t.Run(relation+"-valid-null", func(t *testing.T) {
			request := validRequest()
			request.Observation.ScanContext.Relation = relation
			request.Observation.ScanContext.OpportunityID = nil
			if _, err := Adapt(request); err != nil {
				t.Fatalf("Adapt: %v", err)
			}
		})
	}
}

func TestFrozenEnumsAreAccepted(t *testing.T) {
	for _, depth := range []string{"advisory", "opportunistic", "standard", "thorough"} {
		t.Run("depth-"+depth, func(t *testing.T) {
			request := validRequest()
			request.Observation.ScanContext.Depth = depth
			if _, err := Adapt(request); err != nil {
				t.Fatal(err)
			}
		})
	}
	for _, dimension := range []string{"architecture_drift", "code", "dependencies", "performance", "security", "test_coverage"} {
		t.Run("dimension-"+dimension, func(t *testing.T) {
			request := validRequest()
			request.Observation.ScanContext.Dimension = dimension
			if _, err := Adapt(request); err != nil {
				t.Fatal(err)
			}
		})
	}
	for _, sensitivity := range []string{"confidential", "internal", "public", "restricted"} {
		t.Run("sensitivity-"+sensitivity, func(t *testing.T) {
			request := validRequest()
			request.Binding.Sensitivity = sensitivity
			if _, err := Adapt(request); err != nil {
				t.Fatal(err)
			}
		})
	}
	request := validRequest()
	request.Observation.Producer.ProducerType = "service"
	if _, err := Adapt(request); err != nil {
		t.Fatalf("service producer: %v", err)
	}
}

func TestStandaloneObservationValidationIsPureAndBounded(t *testing.T) {
	observation := validRequest().Observation
	if err := ValidateObservation(observation); err != nil {
		t.Fatalf("ValidateObservation: %v", err)
	}
	observation.Locator.Detail = "bad\u202etext"
	assertErrorContains(t, ValidateObservation(observation), "forbidden Unicode")
}
