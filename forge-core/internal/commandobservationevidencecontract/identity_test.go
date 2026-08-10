package commandobservationevidencecontract

import "testing"

func TestEveryCommandFieldChangesCommandObservationRequestAndEvidenceIdentity(t *testing.T) {
	mutations := []requestMutation{
		{"argv", "", func(r *Request) { r.Observation.Command.Argv = append(r.Observation.Command.Argv, "extra") }},
		{"cwd", "", func(r *Request) { r.Observation.Command.CWD = "." }},
		{"environment", "", func(r *Request) { r.Observation.Command.EnvironmentSHA256 = digestBytes([]byte("environment-2")) }},
		{"stdin", "", func(r *Request) {
			r.Observation.Command.StdinBytes = 1
			r.Observation.Command.StdinSHA256 = digestBytes([]byte("x"))
		}},
		{"timeout", "", func(r *Request) { r.Observation.Command.TimeoutMS = nil }},
		{"tool snapshot", "", func(r *Request) { r.Observation.Command.ToolSnapshotSHA256 = digestBytes([]byte("tool-2")) }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.CommandSHA256 == base.CommandSHA256 ||
				variant.SourceSnapshotSHA256 == base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatalf("command mutation did not alter every downstream identity: %#v", variant)
			}
		})
	}
}

func TestObservationIdentityBoundary(t *testing.T) {
	observationMutations := []requestMutation{
		{"end", "", func(r *Request) { r.Observation.EndedAtUnixMS++ }},
		{"evidence type", "", func(r *Request) { r.Observation.EvidenceType = "test_run" }},
		{"producer id", "", func(r *Request) { r.Observation.Producer.ProducerID = "forge-gate-2" }},
		{"producer type", "", func(r *Request) { r.Observation.Producer.ProducerType = "service" }},
		{"producer version", "", func(r *Request) { r.Observation.Producer.ProducerVersion = "v1.2.4" }},
		{"run", "", func(r *Request) { r.Observation.Producer.RunID = "run-command-0050" }},
		{"revision", "", func(r *Request) { r.Observation.Source.SourceRevision = "680babe" }},
		{"tree", "", func(r *Request) { r.Observation.Source.SourceTreeSHA256 = digestBytes([]byte("tree-2")) }},
		{"start", "", func(r *Request) { r.Observation.StartedAtUnixMS++ }},
		{"retention", "", func(r *Request) {
			r.Observation.Streams.Stdout.RetainedBytes = 4
			r.Observation.Streams.Stdout.RetainedSHA256 = digestBytes([]byte("gate"))
			r.Observation.Streams.Combined.RetainedBytes = 4
			r.Observation.Streams.Combined.RetainedSHA256 = digestBytes([]byte("gate"))
		}},
		{"exit", "", func(r *Request) { r.Observation.Termination.ExitCode = int64Pointer(7) }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range observationMutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.CommandSHA256 != base.CommandSHA256 ||
				variant.SourceSnapshotSHA256 == base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("observation mutation crossed or failed identity boundaries")
			}
		})
	}
}

func TestBindingIdentityBoundary(t *testing.T) {
	bindingMutations := []requestMutation{
		{"aggregate", "", func(r *Request) { r.Binding.AggregateID = "gate-run-command-0050" }},
		{"context", "", func(r *Request) { r.Binding.ContextSHA256 = digestBytes([]byte("context-2")) }},
		{"policy", "", func(r *Request) { r.Binding.PolicySHA256 = digestBytes([]byte("policy-2")) }},
		{"project", "", func(r *Request) { r.Binding.ProjectID = "project-catalyst-2" }},
		{"scope", "", func(r *Request) { r.Binding.Scope = "module:harness" }},
		{"sensitivity", "", func(r *Request) { r.Binding.Sensitivity = "restricted" }},
		{"sequence", "", func(r *Request) { r.Binding.Sequence = 2 }},
		{"subjects", "", func(r *Request) { r.Binding.Subjects = []string{"run:command-0049", "test:other"} }},
		{"supersedes", "", func(r *Request) { r.Binding.SupersedesRecordIDs = []string{"command-evidence-prior"} }},
	}
	base := mustAdapt(t, validRequest())
	for _, mutation := range bindingMutations {
		t.Run(mutation.name, func(t *testing.T) {
			request := validRequest()
			mutation.edit(&request)
			variant := mustAdapt(t, request)
			if variant.CommandSHA256 != base.CommandSHA256 ||
				variant.SourceSnapshotSHA256 != base.SourceSnapshotSHA256 ||
				variant.RequestSHA256 == base.RequestSHA256 ||
				variant.Evidence.Digest() == base.Evidence.Digest() {
				t.Fatal("binding mutation crossed or failed identity boundaries")
			}
		})
	}
}
