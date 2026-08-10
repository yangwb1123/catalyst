package commandobservationevidencecontract

import (
	"math"
	"strings"
	"testing"
)

type requestMutation struct {
	name string
	want string
	edit func(*Request)
}

func TestTypedRequestRejectsInvalidEnvelopeCommandAndBinding(t *testing.T) {
	tests := []requestMutation{
		{"api version", "api_version", func(r *Request) { r.APIVersion = "v2" }},
		{"canonicalization", "canonicalization", func(r *Request) { r.Canonicalization = "json" }},
		{"observation version", "api_version", func(r *Request) { r.Observation.APIVersion = "v2" }},
		{"empty argv", "argv", func(r *Request) { r.Observation.Command.Argv = []string{} }},
		{"empty executable", "argv[0]", func(r *Request) { r.Observation.Command.Argv[0] = "" }},
		{"too many argv", "argv", func(r *Request) { r.Observation.Command.Argv = make([]string, maxArgvItems+1) }},
		{"oversize argument", "Unicode scalars", func(r *Request) { r.Observation.Command.Argv[1] = strings.Repeat("x", maxTextRunes+1) }},
		{"bidi argument", "forbidden Unicode", func(r *Request) { r.Observation.Command.Argv[1] = "bad\u202earg" }},
		{"bad environment hash", "environment_sha256", func(r *Request) { r.Observation.Command.EnvironmentSHA256 = strings.Repeat("A", 64) }},
		{"negative stdin", "stdin_bytes", func(r *Request) { r.Observation.Command.StdinBytes = -1 }},
		{"wrong empty stdin hash", "empty stdin", func(r *Request) { r.Observation.Command.StdinSHA256 = strings.Repeat("1", 64) }},
		{"zero timeout", "timeout_ms", func(r *Request) { r.Observation.Command.TimeoutMS = int64Pointer(0) }},
		{"long timeout", "timeout_ms", func(r *Request) { r.Observation.Command.TimeoutMS = int64Pointer(maxTimeoutMS + 1) }},
		{"negative start", "started_at", func(r *Request) { r.Observation.StartedAtUnixMS = -1 }},
		{"end before start", "cannot precede", func(r *Request) { r.Observation.EndedAtUnixMS = r.Observation.StartedAtUnixMS - 1 }},
		{"evidence type", "evidence_type", func(r *Request) { r.Observation.EvidenceType = "repo_locator" }},
		{"producer id", "producer_id", func(r *Request) { r.Observation.Producer.ProducerID = "Forge Gate" }},
		{"producer type", "producer_type", func(r *Request) { r.Observation.Producer.ProducerType = "human" }},
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

func TestTypedRequestRejectsUnsafeCWD(t *testing.T) {
	for _, value := range []string{"", "/tmp", `C:\\tmp`, `a\\b`, "a//b", "a/./b", "a/../b", "a/", "./a"} {
		t.Run(value, func(t *testing.T) {
			request := validRequest()
			request.Observation.Command.CWD = value
			adaptation, err := Adapt(request)
			if err == nil || adaptation != nil {
				t.Fatalf("unsafe cwd %q accepted: %#v, %v", value, adaptation, err)
			}
		})
	}
	for _, value := range []string{".", "harness", "目录/子目录", "space dir"} {
		t.Run("valid-"+value, func(t *testing.T) {
			request := validRequest()
			request.Observation.Command.CWD = value
			if _, err := Adapt(request); err != nil {
				t.Fatalf("valid cwd %q: %v", value, err)
			}
		})
	}
}

func TestTypedRequestAcceptsServiceProducerAndEverySensitivity(t *testing.T) {
	for _, sensitivity := range []string{"public", "internal", "confidential", "restricted"} {
		t.Run(sensitivity, func(t *testing.T) {
			request := validRequest()
			request.Binding.Sensitivity = sensitivity
			request.Observation.Producer.ProducerType = "service"
			if _, err := Adapt(request); err != nil {
				t.Fatalf("Adapt: %v", err)
			}
		})
	}
}

func TestTypedRequestEnforcesOverallCanonicalByteBudget(t *testing.T) {
	request := validRequest()
	request.Observation.Command.Argv = make([]string, maxArgvItems)
	for index := range request.Observation.Command.Argv {
		request.Observation.Command.Argv[index] = strings.Repeat("x", maxTextRunes)
	}
	adaptation, err := Adapt(request)
	assertErrorContains(t, err, "byte length exceeds")
	if adaptation != nil {
		t.Fatal("oversize request returned partial output")
	}
}

func TestTimeoutAndCancelAreValidObservationsButNotProjectable(t *testing.T) {
	for _, kind := range []string{"timed_out", "cancelled"} {
		t.Run(kind, func(t *testing.T) {
			request := validRequest()
			request.Observation.Termination = Termination{Kind: kind, ExitCode: nil}
			if err := ValidateObservation(request.Observation); err != nil {
				t.Fatalf("ValidateObservation: %v", err)
			}
			adaptation, err := Adapt(request)
			assertErrorContains(t, err, "only exited")
			if adaptation != nil {
				t.Fatal("nonprojectable observation returned partial output")
			}
		})
	}
}

func TestTerminationKindsAndSentinelsFailClosed(t *testing.T) {
	tests := []requestMutation{
		{"exited null", "exited requires", func(r *Request) { r.Observation.Termination.ExitCode = nil }},
		{"negative sentinel", "exited requires", func(r *Request) { r.Observation.Termination.ExitCode = int64Pointer(-1) }},
		{"exit overflow", "exited requires", func(r *Request) { r.Observation.Termination.ExitCode = int64Pointer(maxExitCode + 1) }},
		{"timed out exit", "requires null", func(r *Request) {
			r.Observation.Termination = Termination{Kind: "timed_out", ExitCode: int64Pointer(1)}
		}},
		{"signaled", "unsupported termination", func(r *Request) { r.Observation.Termination = Termination{Kind: "signaled", ExitCode: nil} }},
		{"spawn failed", "unsupported termination", func(r *Request) { r.Observation.Termination = Termination{Kind: "spawn_failed", ExitCode: nil} }},
	}
	runMutationTests(t, tests)
}

func TestStreamCountHashAndTruncationInvariants(t *testing.T) {
	tests := []requestMutation{
		{"combined count", "combined.bytes", func(r *Request) { r.Observation.Streams.Combined.Bytes++ }},
		{"retained too large", "cannot exceed", func(r *Request) { r.Observation.Streams.Stdout.RetainedBytes++ }},
		{"empty full hash", "empty stream", func(r *Request) { r.Observation.Streams.Stderr.SHA256 = strings.Repeat("1", 64) }},
		{"empty retained hash", "empty stream", func(r *Request) { r.Observation.Streams.Stderr.RetainedSHA256 = strings.Repeat("2", 64) }},
		{"fully retained mismatch", "fully retained", func(r *Request) { r.Observation.Streams.Stdout.RetainedSHA256 = strings.Repeat("3", 64) }},
		{"split overflow", "exceeds signed int64", func(r *Request) {
			r.Observation.Streams.Stdout.Bytes = math.MaxInt64
			r.Observation.Streams.Stderr.Bytes = 1
			r.Observation.Streams.Combined.Bytes = math.MaxInt64
		}},
	}
	runMutationTests(t, tests)

	truncated := validRequest()
	truncated.Observation.Streams.Stdout.RetainedBytes = 4
	truncated.Observation.Streams.Stdout.RetainedSHA256 = digestBytes([]byte("gate"))
	truncated.Observation.Streams.Combined.RetainedBytes = 4
	truncated.Observation.Streams.Combined.RetainedSHA256 = digestBytes([]byte("gate"))
	if _, err := Adapt(truncated); err != nil {
		t.Fatalf("valid truncation rejected: %v", err)
	}
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
