package graphterminal

import (
	"bytes"
	"strings"
	"testing"
)

func TestTerminalControlReceiptRoundTripWithoutTrailingLF(t *testing.T) {
	fixture := validTerminalFixture(t)
	if bytes.HasSuffix(fixture.ControlJSON, []byte{'\n'}) ||
		bytes.HasSuffix(fixture.ClaimJSON, []byte{'\n'}) ||
		bytes.HasSuffix(fixture.ArtifactJSON, []byte{'\n'}) ||
		bytes.HasSuffix(fixture.ReceiptJSON, []byte{'\n'}) {
		t.Fatal("canonical terminal fixture has trailing LF")
	}
	if fixture.Receipt.GraphStatus != "completed" || fixture.Receipt.WaveIndex != 0 ||
		!fixture.Receipt.LaneReleaseAuthorized || fixture.Receipt.RetryAuthorized {
		t.Fatalf("unexpected receipt decision: %#v", fixture.Receipt)
	}
}

func TestTerminalControlStrictWireRejections(t *testing.T) {
	valid := string(validTerminalFixture(t).ControlJSON)
	cases := map[string][]byte{
		"duplicate": []byte(strings.Replace(valid, `{"v":1,`, `{"v":1,"v":1,`, 1)),
		"unknown":   []byte(strings.Replace(valid, `{"v":1,`, `{"unknown":0,"v":1,`, 1)),
		"missing":   []byte(strings.Replace(valid, `{"v":1,`, `{`, 1)),
		"nested_duplicate": []byte(strings.Replace(valid,
			`"artifact":{"v":1,`, `"artifact":{"v":1,"v":1,`, 1)),
		"nested_unknown": []byte(strings.Replace(valid,
			`"claim":{"v":1,`, `"claim":{"unknown":0,"v":1,`, 1)),
		"nested_reordered": []byte(strings.Replace(valid,
			`"artifact":{"v":1,"terminal_artifact_protocol_version":1,`,
			`"artifact":{"terminal_artifact_protocol_version":1,"v":1,`, 1)),
		"reordered": []byte(strings.Replace(valid,
			`{"v":1,"scheduler_protocol_version":1,`,
			`{"scheduler_protocol_version":1,"v":1,`, 1)),
		"trailing":    []byte(valid + `x`),
		"trailing_lf": []byte(valid + "\n"),
		"version":     []byte(strings.Replace(valid, `{"v":1,`, `{"v":2,`, 1)),
		"digest":      []byte(strings.Replace(valid, `"snapshot_sha256":"`, `"snapshot_sha256":"0`, 1)),
		"null":        []byte(strings.Replace(valid, `"artifact":{`, `"artifact":null,"discard":{`, 1)),
		"bad_escape":  []byte(strings.Replace(valid, `"output_text":"done"`, `"output_text":"\ud800"`, 1)),
	}
	for name, input := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeControl(bytes.NewReader(input)); err == nil {
				t.Fatal("accepted invalid terminal control")
			}
		})
	}
}

func TestTerminalReceiptFixedOutcomeMappings(t *testing.T) {
	base := validTerminalFixture(t).Control
	assertOutcomeFixture(t, base, "result", "length", "failed")
	for class := range uncertaintyClasses {
		assertOutcomeFixture(t, base, "uncertainty", class, "failed_uncertain")
	}
}

func assertOutcomeFixture(t *testing.T, base TerminalControl, kind, class, expected string) {
	t.Helper()
	base.Artifact.ArtifactKind, base.Artifact.Classification = kind, class
	if class == "missing_usage" {
		base.Artifact.UsageObserved = false
		base.Artifact.InputTokens, base.Artifact.OutputTokens = 0, 0
		base.Artifact.ActualCostCalculated = false
		base.Artifact.ActualCostUSDMicros = 0
	}
	base.Artifact = identifyArtifactFixture(t, base.Artifact)
	base = identifyControlFixture(t, base)
	receipt, err := BuildReceipt(base)
	if err != nil {
		t.Fatalf("%s/%s receipt: %v", kind, class, err)
	}
	if receipt.GraphStatus != expected || receipt.NodeOutcome != expected ||
		receipt.WaveOutcome != expected {
		t.Fatalf("%s/%s outcome = %q, want %q", kind, class, receipt.GraphStatus, expected)
	}
}

func TestTerminalControlRejectsImpossibleUncertaintyEvidence(t *testing.T) {
	base := validTerminalFixture(t).Control
	base.Artifact.ArtifactKind = "uncertainty"
	base.Artifact.Classification = "provider_error"
	mutations := map[string]func(*TerminalArtifact){
		"pre-poll terminal": func(value *TerminalArtifact) {
			value.ProviderPollStarted, value.TerminalSeen = false, true
		},
		"pre-poll eof": func(value *TerminalArtifact) {
			value.ProviderPollStarted, value.StreamEOFSeen = false, true
		},
		"missing usage observed": func(value *TerminalArtifact) {
			value.Classification = "missing_usage"
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			value := base
			mutate(&value.Artifact)
			value.Artifact = identifyArtifactFixture(t, value.Artifact)
			value = identifyControlFixture(t, value)
			if _, err := BuildReceipt(value); err == nil {
				t.Fatal("accepted impossible uncertainty evidence")
			}
		})
	}
}

func TestMarshalReceiptRejectsResignedArtifactAndOutcomeDrift(t *testing.T) {
	base := validTerminalFixture(t).Receipt
	mutations := map[string]func(*Receipt){
		"artifact identity": func(value *Receipt) { value.ArtifactID = "other-artifact" },
		"artifact outcome": func(value *Receipt) {
			value.ArtifactKind = "uncertainty"
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			value := base
			mutate(&value)
			resignReceiptFixture(t, &value)
			if _, err := MarshalReceipt(value); err == nil {
				t.Fatal("accepted resigned terminal receipt drift")
			}
		})
	}
}

func resignReceiptFixture(t *testing.T, value *Receipt) {
	t.Helper()
	value.ReceiptSHA256 = mustDigestFixture(t, receiptDigestDomain, payloadFromReceipt(*value))
	value.ReceiptID = "graph-node-terminal-receipt-" + value.ReceiptSHA256
}

func TestTerminalControlRejectsBindingAndCostDrift(t *testing.T) {
	base := validTerminalFixture(t).Control
	mutations := map[string]func(*TerminalControl){
		"claim_head":      func(value *TerminalControl) { value.Claim.ClaimEventSHA256 = repeatedDigest('0') },
		"lane_owner":      func(value *TerminalControl) { value.ActiveLane.LaneOwnershipID = "other-owner" },
		"artifact_output": func(value *TerminalControl) { value.Artifact.OutputText = "drift" },
		"actual_cost":     func(value *TerminalControl) { value.Artifact.ActualCostUSDMicros++ },
		"artifact_time":   func(value *TerminalControl) { value.Artifact.CreatedAtMS = value.Claim.ReleasedAtMS - 1 },
		"pricing":         func(value *TerminalControl) { value.Pricing.InputUSDMicrosPerTokenUnit++ },
		"multi_node":      func(value *TerminalControl) { value.Plan.AuthoredNodeIDs = append(value.Plan.AuthoredNodeIDs, "other") },
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			value := base
			mutate(&value)
			if name == "actual_cost" || name == "artifact_time" {
				value.Artifact = identifyArtifactFixture(t, value.Artifact)
			}
			value = identifyControlFixture(t, value)
			if _, err := BuildReceipt(value); err == nil {
				t.Fatal("accepted drift")
			}
		})
	}
}
