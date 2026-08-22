package graphsnapshot

import (
	"strings"
	"testing"
)

func TestJSONWalkerUsesDedicatedResolvedEdgeUnionLimit(t *testing.T) {
	atLimit := syntheticArrayEnvelope("edges", maxEdges)
	if err := validateEnvelopeJSONShape(atLimit); err != nil {
		t.Fatalf("dedicated %d edge bound rejected: %v", maxEdges, err)
	}
	if err := validateEnvelopeJSONShape(syntheticArrayEnvelope("edges", maxEdges+1)); err == nil {
		t.Fatal("resolved edge union N+1 was accepted")
	}
	if err := validateEnvelopeJSONShape(syntheticArrayEnvelope("other", defaultArrayLimit+1)); err == nil {
		t.Fatal("generic array walker inherited the edge union limit")
	}
}

func TestJSONWalkerEnforcesAggregateSourceLocatorsBeforeDecode(t *testing.T) {
	atLimit := syntheticLocatorEnvelope(maxAggregateLocators)
	if err := validateEnvelopeJSONShape(atLimit); err != nil {
		t.Fatalf("aggregate locator bound rejected: %v", err)
	}
	overLimit := syntheticLocatorEnvelope(maxAggregateLocators + 1)
	if _, err := Decode(overLimit); err == nil ||
		!strings.Contains(err.Error(), "aggregate source locators") {
		t.Fatalf("aggregate locator N+1 error = %v", err)
	}
}

func TestIdentifierAndInputBoundsFailBeforeOutput(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	for _, projectID := range []string{"", "Upper", strings.Repeat("a", 161)} {
		if production, err := Build(graph, digest, observation.Producer.RunID, projectID); err == nil || production != nil {
			t.Fatalf("project ID %q accepted", projectID)
		}
	}
	if production, err := Build(make([]byte, maxGraphBytes+1), digest,
		observation.Producer.RunID, "bounded-project"); err == nil || production != nil {
		t.Fatal("oversized graph produced output")
	}
}

func syntheticArrayEnvelope(field string, count int) []byte {
	var builder strings.Builder
	builder.WriteString(`{"snapshot":{"` + field + `":[`)
	for index := 0; index < count; index++ {
		if index != 0 {
			builder.WriteByte(',')
		}
		builder.WriteByte('0')
	}
	builder.WriteString(`]}}`)
	return []byte(builder.String())
}

func syntheticLocatorEnvelope(count int) []byte {
	var builder strings.Builder
	builder.WriteString(`{"snapshot":{"nodes":[`)
	for record, remaining := 0, count; remaining > 0; record++ {
		if record != 0 {
			builder.WriteByte(',')
		}
		chunk := maxLocators
		if remaining < chunk {
			chunk = remaining
		}
		builder.WriteString(`{"source_locators":[`)
		for index := 0; index < chunk; index++ {
			if index != 0 {
				builder.WriteByte(',')
			}
			builder.WriteByte('0')
		}
		builder.WriteString(`]}`)
		remaining -= chunk
	}
	builder.WriteString(`]}}`)
	return []byte(builder.String())
}
