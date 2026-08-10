package commandobservationevidencecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
	"testing"
)

func int64Pointer(value int64) *int64 { return &value }

func digestBytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func validRequest() Request {
	stdout := []byte("gate ok\n")
	stdoutDigest := digestBytes(stdout)
	return Request{
		APIVersion: APIVersion,
		Binding: Binding{
			AggregateID: "gate-run-command-0049", ContextSHA256: digestBytes([]byte("context")),
			PolicySHA256: digestBytes([]byte("policy")), ProjectID: "project-catalyst",
			Scope: "project", Sensitivity: "internal", Sequence: 1,
			Subjects: []string{"run:command-0049", "test:harness"}, SupersedesRecordIDs: []string{},
		},
		Canonicalization: Canonicalization,
		Observation: Observation{
			APIVersion: ObservationAPIVersion, Canonicalization: Canonicalization,
			Command: Command{
				Argv: []string{"python3", "-m", "unittest", ""}, CWD: "harness",
				EnvironmentSHA256: digestBytes([]byte("environment")), StdinBytes: 0,
				StdinSHA256: emptySHA256, TimeoutMS: int64Pointer(60000),
				ToolSnapshotSHA256: digestBytes([]byte("tool-snapshot")),
			},
			EndedAtUnixMS: 1786354860123, EvidenceType: "gate_result",
			Producer: Producer{
				ProducerID: "forge-gate", ProducerType: "tool",
				ProducerVersion: "v1.2.3", RunID: "run-command-0049",
			},
			Source: Source{
				SourceRevision: "680babd", SourceTreeSHA256: digestBytes([]byte("source-tree")),
			},
			StartedAtUnixMS: 1786354800000,
			Streams: Streams{
				Combined: Stream{Bytes: int64(len(stdout)), RetainedBytes: int64(len(stdout)), RetainedSHA256: stdoutDigest, SHA256: stdoutDigest},
				Stderr:   Stream{Bytes: 0, RetainedBytes: 0, RetainedSHA256: emptySHA256, SHA256: emptySHA256},
				Stdout:   Stream{Bytes: int64(len(stdout)), RetainedBytes: int64(len(stdout)), RetainedSHA256: stdoutDigest, SHA256: stdoutDigest},
			},
			Termination: Termination{ExitCode: int64Pointer(0), Kind: "exited"},
		},
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

func mustAdapt(t *testing.T, request Request) *Adaptation {
	t.Helper()
	adaptation, err := Adapt(request)
	if err != nil {
		t.Fatalf("Adapt: %v", err)
	}
	return adaptation
}

func assertErrorContains(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf("error = %v, want substring %q", err, want)
	}
}

func repeatedIDs(prefix string, count int) []string {
	result := make([]string, count)
	for index := range result {
		result[index] = fmt.Sprintf("%s-%03d", prefix, index)
	}
	return result
}
