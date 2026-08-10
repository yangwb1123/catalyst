package evolverepolocatorevidencecontract

import "testing"

func TestEveryLocatorFieldChangesEveryDownstreamIdentity(t *testing.T) {
	mutations := []struct {
		name string
		edit func(*Request)
	}{
		{"detail", func(r *Request) { r.Observation.Locator.Detail += " changed" }},
		{"line", func(r *Request) { r.Observation.Locator.Line++ }},
		{"path", func(r *Request) { r.Observation.Locator.Path = ".arch/other.yaml" }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.LocatorSHA256 == base.LocatorSHA256 ||
				variant.SourceSnapshotSHA256 == base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("locator mutation did not alter every downstream identity")
			}
		})
	}
}

func TestObservationIdentityBoundary(t *testing.T) {
	mutations := []struct {
		name string
		edit func(*Request)
	}{
		{"content bytes", func(r *Request) { r.Observation.Content.Bytes++ }},
		{"content hash", func(r *Request) { r.Observation.Content.SHA256 = digestBytes([]byte("content-2")) }},
		{"observed time", func(r *Request) { r.Observation.ObservedAtUnixMS++ }},
		{"producer parameters", func(r *Request) { r.Observation.Producer.ParametersSHA256 = digestBytes([]byte("parameters-2")) }},
		{"producer id", func(r *Request) { r.Observation.Producer.ProducerID = "forgeos.fixture-evolve-scanner-2" }},
		{"producer type", func(r *Request) { r.Observation.Producer.ProducerType = "service" }},
		{"producer version", func(r *Request) { r.Observation.Producer.ProducerVersion = "v2" }},
		{"run id", func(r *Request) { r.Observation.Producer.RunID = "run-evolve-0051" }},
		{"depth", func(r *Request) { r.Observation.ScanContext.Depth = "standard" }},
		{"dimension", func(r *Request) { r.Observation.ScanContext.Dimension = "security" }},
		{"relation", func(r *Request) {
			r.Observation.ScanContext.Relation = "finding"
			r.Observation.ScanContext.OpportunityID = nil
		}},
		{"opportunity", func(r *Request) { r.Observation.ScanContext.OpportunityID = stringPointer("architecture-budget-0051") }},
		{"report", func(r *Request) { r.Observation.ScanContext.ReportSHA256 = digestBytes([]byte("report-2")) }},
		{"revision", func(r *Request) { r.Observation.Source.SourceRevision = "fixture-revision-0051" }},
		{"tree", func(r *Request) { r.Observation.Source.SourceTreeSHA256 = digestBytes([]byte("tree-2")) }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.LocatorSHA256 != base.LocatorSHA256 ||
				variant.SourceSnapshotSHA256 == base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("observation mutation crossed or failed identity boundaries")
			}
		})
	}
}

func TestBindingIdentityBoundary(t *testing.T) {
	mutations := []struct {
		name string
		edit func(*Request)
	}{
		{"aggregate", func(r *Request) { r.Binding.AggregateID = "evolve-scan-run-0051" }},
		{"context", func(r *Request) { r.Binding.ContextSHA256 = digestBytes([]byte("context-2")) }},
		{"policy", func(r *Request) { r.Binding.PolicySHA256 = digestBytes([]byte("policy-2")) }},
		{"project", func(r *Request) { r.Binding.ProjectID = "project-catalyst-2" }},
		{"scope", func(r *Request) { r.Binding.Scope = "module:harness" }},
		{"sensitivity", func(r *Request) { r.Binding.Sensitivity = "restricted" }},
		{"sequence", func(r *Request) { r.Binding.Sequence = 2 }},
		{"subjects", func(r *Request) { r.Binding.Subjects = []string{"evolve:architecture-drift", "run:evolve-0051"} }},
		{"supersedes", func(r *Request) { r.Binding.SupersedesRecordIDs = []string{"evolve-locator-evidence-prior"} }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.LocatorSHA256 != base.LocatorSHA256 ||
				variant.SourceSnapshotSHA256 != base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("binding mutation crossed or failed identity boundaries")
			}
		})
	}
}

func TestIdentityDomainsAreDistinct(t *testing.T) {
	adaptation := mustAdapt(t, validRequest())
	if adaptation.LocatorSHA256 == adaptation.SourceSnapshotSHA256 ||
		adaptation.LocatorSHA256 == adaptation.RequestSHA256 ||
		adaptation.SourceSnapshotSHA256 == adaptation.RequestSHA256 ||
		adaptation.Evidence.Digest() == adaptation.RequestSHA256 {
		t.Fatal("domain-separated identities collapsed")
	}
}
