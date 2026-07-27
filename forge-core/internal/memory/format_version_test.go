package memory

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestMemoryFormatRejectsUnknownGeneration(t *testing.T) {
	line := []byte(`{"_format":"forgeos.memory.v2","kind":"gap","topic":"x","detail":"y"}` + "\n")
	if _, err := decode(line); err == nil || !strings.Contains(err.Error(), "unsupported memory format") {
		t.Fatalf("decode unknown format error = %v", err)
	}

	path := filepath.Join(t.TempDir(), "memory.jsonl")
	err := Append(path, Entry{Format: "forgeos.memory.v2", Kind: KindGap, Topic: "x", Detail: "y"})
	if err == nil || !strings.Contains(err.Error(), "unsupported memory format") {
		t.Fatalf("Append unknown format error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("unsupported memory entry must not be written; stat err=%v", statErr)
	}
}

func TestMemoryFormatAcceptsLegacyEmptyMarker(t *testing.T) {
	line := []byte(`{"kind":"lesson","topic":"x","detail":"legacy"}` + "\n")
	entries, err := decode(line)
	if err != nil || len(entries) != 1 || entries[0].Detail != "legacy" {
		t.Fatalf("legacy entries = %+v, %v", entries, err)
	}
}

func TestExplicitZeroConfidenceSurvivesRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	want := Entry{
		Kind: KindDecision, Topic: "unsafe-choice", Detail: "contradicted",
		Confidence: 0, ConfidenceExplicitZero: true,
	}
	if err := Append(path, want); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), `"confidence":0`) {
		t.Fatalf("explicit zero was omitted: %s", raw)
	}
	got, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got[0].Confidence != 0 || !got[0].ConfidenceExplicitZero {
		t.Fatalf("explicit zero confidence did not survive: %+v", got)
	}
}

func TestOmittedConfidenceStillDefaultsToOne(t *testing.T) {
	entries, err := decode([]byte(`{"kind":"lesson","topic":"x","detail":"legacy"}` + "\n"))
	if err != nil || len(entries) != 1 || entries[0].Confidence != 1 ||
		entries[0].ConfidenceExplicitZero {
		t.Fatalf("legacy confidence default = %+v, %v", entries, err)
	}
}
