package evolverepolocatorevidencecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
	"testing"
)

func stringPointer(value string) *string { return &value }

func digestBytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func validRequest() Request {
	return Request{
		APIVersion: APIVersion,
		Binding: Binding{
			AggregateID: "evolve-scan-run-0050", ContextSHA256: digestBytes([]byte("context")),
			PolicySHA256: digestBytes([]byte("policy")), ProjectID: "project-catalyst",
			Scope: "project", Sensitivity: "internal", Sequence: 1,
			Subjects:            []string{"evolve:architecture-drift", "run:evolve-0050"},
			SupersedesRecordIDs: []string{},
		},
		Canonicalization: Canonicalization,
		Observation: Observation{
			APIVersion: ObservationAPIVersion, Canonicalization: Canonicalization,
			Content: Content{Bytes: 169, SHA256: digestBytes([]byte("repository content"))},
			Locator: Locator{
				Detail: "The architecture budget is a repository-local scan input.",
				Line:   114, Path: ".arch/rules.yaml",
			},
			ObservedAtUnixMS: 1786345200123,
			Producer: Producer{
				ParametersSHA256: digestBytes([]byte("parameters")),
				ProducerID:       "forgeos.fixture-evolve-scanner", ProducerType: "tool",
				ProducerVersion: "v1", RunID: "run-evolve-0050",
			},
			ScanContext: ScanContext{
				Contract: ScanContract, Depth: "thorough", Dimension: "architecture_drift",
				OpportunityID: stringPointer("architecture-budget-0050"), Relation: "opportunity",
				ReportSHA256: digestBytes([]byte("report")),
			},
			Source: Source{
				SourceRevision:   "fixture-revision-0050",
				SourceTreeSHA256: digestBytes([]byte("source-tree")),
			},
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

type requestMutation struct {
	name string
	want string
	edit func(*Request)
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
