package routing

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// writeCards marshals cards to a temp scorecards.json and returns its path. The
// round-trip through real JSON (not a hand-rolled literal) is deliberate: it
// pins the wire contract — struct tags must survive marshal+unmarshal — so a tag
// rename that drifts from the shared scorecards.json contract fails here.
func writeCards(t *testing.T, cards []Scorecard) string {
	t.Helper()
	data, err := json.Marshal(cards)
	if err != nil {
		t.Fatalf("marshal cards: %v", err)
	}
	path := filepath.Join(t.TempDir(), "scorecards.json")
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
	return path
}

func TestLoadScorecards_RoundTrip(t *testing.T) {
	want := []Scorecard{
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.86, Samples: 142,
			UpdatedAt: "2026-06-01T00:00:00Z", PassRate: 0.78, AvgIterations: 1.4, ReworkRate: 0.05},
		{Model: Opus, TaskType: "architecture", QualityScore: 0.91, Samples: 37,
			UpdatedAt: "2026-06-10T00:00:00Z"},
	}
	got, err := LoadScorecards(writeCards(t, want))
	if err != nil {
		t.Fatalf("LoadScorecards: unexpected error %v", err)
	}
	if len(got) != len(want) {
		t.Fatalf("loaded %d cards, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("card[%d] = %+v, want %+v", i, got[i], want[i])
		}
	}
}

func TestLoadScorecards_MissingFileIsColdStart(t *testing.T) {
	// Cold start: a never-written scorecards file is normal, NOT an error.
	path := filepath.Join(t.TempDir(), "does-not-exist.json")
	got, err := LoadScorecards(path)
	if err != nil {
		t.Fatalf("missing file should be (nil,nil), got err %v", err)
	}
	if got != nil {
		t.Errorf("missing file should yield nil slice, got %+v", got)
	}
}

func TestLoadScorecards_EmptyArrayIsColdStart(t *testing.T) {
	// Syntactically valid but empty == cold start, normalized to nil.
	got, err := LoadScorecards(writeCards(t, []Scorecard{}))
	if err != nil {
		t.Fatalf("empty array should be (nil,nil), got err %v", err)
	}
	if got != nil {
		t.Errorf("empty array should yield nil slice, got %+v", got)
	}
}

func TestLoadScorecards_MalformedIsError(t *testing.T) {
	// A corrupt file is a real fault and must be surfaced, never swallowed as
	// "no history" — that would mask a broken Eval producer.
	path := filepath.Join(t.TempDir(), "bad.json")
	if err := os.WriteFile(path, []byte("{ this is not json ]"), 0o644); err != nil {
		t.Fatalf("write bad file: %v", err)
	}
	got, err := LoadScorecards(path)
	if err == nil {
		t.Fatalf("malformed JSON should error, got nil (cards=%+v)", got)
	}
	if got != nil {
		t.Errorf("malformed JSON should yield nil slice, got %+v", got)
	}
}

func TestLookup(t *testing.T) {
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.86, Samples: 142},
		{Model: Opus, TaskType: "implementation", QualityScore: 0.90, Samples: 50},
		{Model: Sonnet, TaskType: "bugfix", QualityScore: 0.70, Samples: 30},
	}

	// Hit: exact (model, task_type) primary key.
	if c, ok := Lookup(cards, Opus, "implementation"); !ok || c.QualityScore != 0.90 {
		t.Errorf("Lookup(opus,implementation) = (%+v,%v), want the 0.90 card, true", c, ok)
	}
	// Hit: same model, different task_type resolves independently.
	if c, ok := Lookup(cards, Sonnet, "bugfix"); !ok || c.Samples != 30 {
		t.Errorf("Lookup(sonnet,bugfix) = (%+v,%v), want the 30-sample card, true", c, ok)
	}
	// Miss: pair absent -> zero value + false.
	if c, ok := Lookup(cards, Haiku, "implementation"); ok || c != (Scorecard{}) {
		t.Errorf("Lookup(haiku,implementation) = (%+v,%v), want (zero,false)", c, ok)
	}
	// Miss: right model, wrong task_type.
	if _, ok := Lookup(cards, Opus, "architecture"); ok {
		t.Error("Lookup(opus,architecture) = true, want false (no such pair)")
	}
}

func TestHistoryTiebreak_PicksHighestQualifyingScore(t *testing.T) {
	// Multi-candidate band (a v3-shaped scenario, exercised today to prove the
	// selection logic): all three meet min_samples; opus has the top quality.
	cards := []Scorecard{
		{Model: Haiku, TaskType: "implementation", QualityScore: 0.70, Samples: 50},
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.86, Samples: 50},
		{Model: Opus, TaskType: "implementation", QualityScore: 0.91, Samples: 50},
	}
	got, reason := HistoryTiebreak([]string{Haiku, Sonnet, Opus}, "implementation", cards, 20)
	if got != Opus {
		t.Errorf("tiebreak picked %q, want %q (highest quality)", got, Opus)
	}
	if want := "picked opus by quality 0.91 (50 samples)"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
}

func TestHistoryTiebreak_SkipsThinThenPicksQualifying(t *testing.T) {
	// The highest raw score (opus 0.99) is below min_samples and must be ignored;
	// the best score among the SUFFICIENTLY-sampled candidates (sonnet) wins.
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "bugfix", QualityScore: 0.80, Samples: 40},
		{Model: Opus, TaskType: "bugfix", QualityScore: 0.99, Samples: 5}, // too thin
	}
	got, reason := HistoryTiebreak([]string{Sonnet, Opus}, "bugfix", cards, 20)
	if got != Sonnet {
		t.Errorf("tiebreak picked %q, want %q (opus is under min_samples)", got, Sonnet)
	}
	if want := "picked sonnet by quality 0.80 (40 samples)"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
}

func TestHistoryTiebreak_AllUnderMinFallsBack(t *testing.T) {
	// Scorecards exist but none clears min_samples -> distrust noise, fall back to
	// the tier default (candidates[0]), with an insufficient-samples reason.
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.95, Samples: 3},
		{Model: Opus, TaskType: "implementation", QualityScore: 0.99, Samples: 10},
	}
	got, reason := HistoryTiebreak([]string{Sonnet, Opus}, "implementation", cards, 20)
	if got != Sonnet { // candidates[0] is the tier default
		t.Errorf("tiebreak = %q, want %q (tier_default fallback)", got, Sonnet)
	}
	if want := "insufficient samples (<20) -> tier_default"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
}

func TestHistoryTiebreak_NoScorecardFallsBack(t *testing.T) {
	// Cold start for this task_type: no candidate has any history -> tier default.
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "bugfix", QualityScore: 0.80, Samples: 40}, // different task_type
	}
	got, reason := HistoryTiebreak([]string{Sonnet, Opus}, "implementation", cards, 20)
	if got != Sonnet { // candidates[0]
		t.Errorf("tiebreak = %q, want %q (no-scorecard fallback)", got, Sonnet)
	}
	if want := "no scorecard -> tier_default"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
	// Same path when cards is entirely empty (whole-system cold start).
	if got, reason := HistoryTiebreak([]string{Sonnet}, "implementation", nil, 20); got != Sonnet ||
		reason != "no scorecard -> tier_default" {
		t.Errorf("nil cards = (%q,%q), want (sonnet, no-scorecard)", got, reason)
	}
}

func TestHistoryTiebreak_EmptyCandidates(t *testing.T) {
	// No candidates at all -> empty pick, distinct reason (caller uses tier_default).
	got, reason := HistoryTiebreak(nil, "implementation", nil, 20)
	if got != "" {
		t.Errorf("empty candidates pick = %q, want \"\"", got)
	}
	if want := "no candidates -> tier_default"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
}

func TestHistoryTiebreak_TieBrokenByOrder(t *testing.T) {
	// Equal quality_score: the first candidate wins (strictly-greater compare),
	// so v1's single-candidate-per-tier reality stays deterministic.
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.88, Samples: 30},
		{Model: Opus, TaskType: "implementation", QualityScore: 0.88, Samples: 30},
	}
	got, _ := HistoryTiebreak([]string{Sonnet, Opus}, "implementation", cards, 20)
	if got != Sonnet {
		t.Errorf("tie = %q, want %q (first candidate on equal quality)", got, Sonnet)
	}
}

func TestHistoryTiebreak_SingleCandidateV1Passthrough(t *testing.T) {
	// The dominant v1 shape: one candidate in the band. It passes through, and the
	// reason makes the (now-observable) history explicit.
	cards := []Scorecard{
		{Model: Sonnet, TaskType: "implementation", QualityScore: 0.86, Samples: 142},
	}
	got, reason := HistoryTiebreak([]string{Sonnet}, "implementation", cards, 20)
	if got != Sonnet {
		t.Errorf("single-candidate tiebreak = %q, want %q", got, Sonnet)
	}
	if want := "picked sonnet by quality 0.86 (142 samples)"; reason != want {
		t.Errorf("reason = %q, want %q", reason, want)
	}
}

func TestCandidatesForTier(t *testing.T) {
	cases := []struct {
		tier string
		want []string
	}{
		{Haiku, []string{Haiku}},
		{Sonnet, []string{Sonnet, Haiku}},
		{Opus, []string{Opus, Sonnet, Haiku}},
		{"unknown", []string{"unknown"}}, // unknown tier: single-element passthrough
	}
	for _, c := range cases {
		got := CandidatesForTier(c.tier)
		if len(got) != len(c.want) {
			t.Errorf("CandidatesForTier(%q) = %v, want %v", c.tier, got, c.want)
			continue
		}
		for i, g := range got {
			if g != c.want[i] {
				t.Errorf("CandidatesForTier(%q)[%d] = %q, want %q", c.tier, i, g, c.want[i])
			}
		}
	}
}

func TestIsOpusFloorAgent(t *testing.T) {
	floor := []string{"reviewer", "architect", "cto"}
	for _, a := range floor {
		if !IsOpusFloorAgent(a) {
			t.Errorf("IsOpusFloorAgent(%q) = false, want true (safety floor)", a)
		}
	}
	nonFloor := []string{"implementer", "planner", "scanner", "qa", "harness", ""}
	for _, a := range nonFloor {
		if IsOpusFloorAgent(a) {
			t.Errorf("IsOpusFloorAgent(%q) = true, want false", a)
		}
	}
}
