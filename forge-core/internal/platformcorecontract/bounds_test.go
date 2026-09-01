package platformcorecontract

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestPayloadByteBoundary(t *testing.T) {
	value := loadGoldenFixture(t).CommandEnvelope
	value.Payload = map[string]any{
		"a": strings.Repeat("x", maxStringBytes),
		"b": strings.Repeat("x", maxStringBytes-15),
	}
	if canonical, err := canonicalJSON(value.Payload, maxPayloadBytes); err != nil || len(canonical) != maxPayloadBytes {
		t.Fatalf("payload N boundary: len %d, err %v", len(canonical), err)
	}
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("payload N rejected: %v", err)
	}
	value.Payload["b"] = strings.Repeat("x", maxStringBytes-14)
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("payload N+1 accepted")
	}
}

func TestCollectionAndStringBoundaries(t *testing.T) {
	value := loadGoldenFixture(t).CommandEnvelope
	array := make([]any, maxArrayItems)
	value.Payload = map[string]any{"items": array}
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("array N rejected: %v", err)
	}
	value.Payload = map[string]any{"items": append(array, nil)}
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("array N+1 accepted")
	}
	value = loadGoldenFixture(t).CommandEnvelope
	value.Payload = map[string]any{"text": strings.Repeat("x", maxStringBytes)}
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("string N rejected: %v", err)
	}
	value.Payload["text"] = strings.Repeat("x", maxStringBytes+1)
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("string N+1 accepted")
	}
}

func TestExtensionCountBoundary(t *testing.T) {
	value := loadGoldenFixture(t).CommandEnvelope
	value.Extensions = make(map[string]any)
	for index := 0; index < maxExtensionFields; index++ {
		value.Extensions[extensionKey(index)] = int64(index)
	}
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("extension N rejected: %v", err)
	}
	value.Extensions[extensionKey(maxExtensionFields)] = int64(17)
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("extension N+1 accepted")
	}
}

func TestObjectDepthAndExtensionByteBoundaries(t *testing.T) {
	value := loadGoldenFixture(t).CommandEnvelope
	object := make(map[string]any)
	for index := 0; index < maxObjectFields; index++ {
		object["field_"+string(rune('a'+index%26))+string(rune('a'+index/26))] = nil
	}
	value.Payload = object
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("object N rejected: %v", err)
	}
	object["field_extra"] = nil
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("object N+1 accepted")
	}
	value = loadGoldenFixture(t).CommandEnvelope
	value.Extensions = map[string]any{"fixture.large": strings.Repeat("x", maxExtensionsBytes)}
	if err := ValidateCommandEnvelope(&value); err == nil {
		t.Fatal("oversized extensions accepted")
	}
}

func TestWholeEnvelopeDepthForPayloadsAndExtensions(t *testing.T) {
	for _, arrays := range []int{maxJSONDepth - 3, maxJSONDepth - 2} {
		accepted := arrays == maxJSONDepth-3
		nested := nestedArrays(arrays)

		command := loadGoldenFixture(t).CommandEnvelope
		command.Payload = map[string]any{"nested": nested}
		command.PayloadArtifactRef = nil
		assertCommandDepth(t, &command, accepted)
		command = loadGoldenFixture(t).CommandEnvelope
		command.Extensions = map[string]any{"fixture.nested": nested}
		assertCommandDepth(t, &command, accepted)

		event := loadGoldenFixture(t).EventEnvelope
		event.Payload = map[string]any{"nested": nested}
		event.PayloadArtifactRef = nil
		assertEventDepth(t, &event, accepted)
		event = loadGoldenFixture(t).EventEnvelope
		event.Extensions = map[string]any{"fixture.nested": nested}
		assertEventDepth(t, &event, accepted)
	}
}

func nestedArrays(count int) any {
	var value any
	for range count {
		value = []any{value}
	}
	return value
}

func assertCommandDepth(t *testing.T, value *CommandEnvelope, accepted bool) {
	t.Helper()
	validationErr := ValidateCommandEnvelope(value)
	encoded, writerErr := CanonicalCommandEnvelopeJSON(value)
	if (validationErr == nil) != accepted || (writerErr == nil) != accepted {
		t.Fatalf("command depth accepted=%v, validator=%v, writer=%v", accepted, validationErr, writerErr)
	}
	if accepted {
		if _, err := DecodeCanonicalCommandEnvelope(encoded); err != nil {
			t.Fatalf("accepted command did not round trip: %v", err)
		}
		return
	}
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalCommandEnvelope(raw); err == nil || !strings.Contains(err.Error(), "depth") {
		t.Fatalf("invalid command decoder error = %v", err)
	}
}

func assertEventDepth(t *testing.T, value *EventEnvelope, accepted bool) {
	t.Helper()
	validationErr := ValidateEventEnvelope(value)
	encoded, writerErr := CanonicalEventEnvelopeJSON(value)
	if (validationErr == nil) != accepted || (writerErr == nil) != accepted {
		t.Fatalf("event depth accepted=%v, validator=%v, writer=%v", accepted, validationErr, writerErr)
	}
	if accepted {
		if _, err := DecodeCanonicalEventEnvelope(encoded); err != nil {
			t.Fatalf("accepted event did not round trip: %v", err)
		}
		return
	}
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalEventEnvelope(raw); err == nil || !strings.Contains(err.Error(), "depth") {
		t.Fatalf("invalid event decoder error = %v", err)
	}
}

func extensionKey(index int) string {
	return "fixture.key_" + string(rune('a'+index))
}

func TestPlatformIDBoundaries(t *testing.T) {
	valid := []string{
		"spc_00000000000000000000000000",
		"act_7zzzzzzzzzzzzzzzzzzzzzzzzz",
	}
	for _, value := range valid {
		if err := ValidatePlatformID(value); err != nil {
			t.Fatalf("valid ID %q rejected: %v", value, err)
		}
	}
	invalid := []string{
		"spc_80000000000000000000000000",
		"spc_0000000000000000000000000i",
		"run_00000000000000000000000001",
		"spc_0000000000000000000000001",
	}
	for _, value := range invalid {
		if err := ValidatePlatformID(value); err == nil {
			t.Fatalf("invalid ID %q accepted", value)
		}
	}
}

func TestOversizedDocumentFailsBeforeDecode(t *testing.T) {
	raw := []byte(`{"payload":"` + strings.Repeat("x", maxEnvelopeBytes) + `"}`)
	if _, err := DecodeCanonicalCommandEnvelope(raw); err == nil {
		t.Fatal("oversized document accepted")
	}
}

func TestCanonicalWriterRejectsTypedNilAndHighFanout(t *testing.T) {
	value := loadGoldenFixture(t).CommandEnvelope
	var nilObject map[string]any
	var nilArray []any
	for name, child := range map[string]any{"object": nilObject, "array": nilArray} {
		t.Run(name, func(t *testing.T) {
			value.Payload = map[string]any{"nested": child}
			if _, err := CanonicalCommandEnvelopeJSON(&value); err == nil {
				t.Fatal("typed nil container was accepted")
			}
		})
	}
	value = loadGoldenFixture(t).CommandEnvelope
	value.Extensions = map[string]any{"fixture.nil": nilObject}
	if _, err := CanonicalCommandEnvelopeJSON(&value); err == nil {
		t.Fatal("typed nil extension object was accepted")
	}
	value.Extensions = map[string]any{}
	value.Payload = map[string]any{"nested": nil}
	encoded, err := CanonicalCommandEnvelopeJSON(&value)
	if err != nil {
		t.Fatalf("ordinary JSON null rejected: %v", err)
	}
	if _, err := DecodeCanonicalCommandEnvelope(encoded); err != nil {
		t.Fatalf("writer output did not round trip: %v", err)
	}
	shared := make([]any, maxArrayItems)
	fanout := make([]any, maxArrayItems)
	for index := range fanout {
		fanout[index] = shared
	}
	if _, err := canonicalJSON(map[string]any{"fanout": fanout}, maxPayloadBytes); err == nil {
		t.Fatal("high-fanout payload exceeded no aggregate/output budget")
	}
}
