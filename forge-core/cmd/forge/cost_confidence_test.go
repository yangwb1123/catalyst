package main

import "testing"

// cost_confidence_test.go — parseConfidenceScore, the THIRD verdict-shaped parser
// (after parseReviewerVerdict/parseExecutiveVerdict, tested in cost_test.go). Kept
// in its own file (cost_test.go is already near the 500-line volume budget) rather
// than growing that file further.

// parseConfidenceScore on a well-formed "CONFIDENCE: <N>" last line, for N spanning
// the boundary and midpoint of the contracted [0,100] range, must return the exact
// numeric value with ok=true — the mirror of TestParseReviewerVerdict_ApproveAndRequestChanges
// / TestParseExecutiveVerdict_AllFiveTokens, but for product-manager.md's numeric
// requirement-discovery contract.
func TestParseConfidenceScore_ValidBoundaryAndMidpoint(t *testing.T) {
	cases := []struct {
		last string
		want float64
	}{
		{"CONFIDENCE: 0", 0},
		{"CONFIDENCE: 50", 50},
		{"CONFIDENCE: 100", 100},
	}
	for _, c := range cases {
		out := "需求分析...\n缺失信息:无\n" + c.last
		score, ok := parseConfidenceScore(out)
		if !ok || score != c.want {
			t.Errorf("parseConfidenceScore(%q) = (%v,%v), want (%v,true)", c.last, score, ok, c.want)
		}
	}
}

// HONESTY (mirrors parseReviewerVerdict/parseExecutiveVerdict's malformed tests): an
// out-of-range, non-integer, missing, or wrapped confidence must yield ok=false —
// never a fabricated score. This is the case that keeps discover.yml's
// requirement_confidence >= 80 stop condition from ever being silently satisfied by
// a bogus signal.
func TestParseConfidenceScore_OutOfRangeOrMalformedIsNotOK(t *testing.T) {
	for _, out := range []string{
		"",
		"   ",
		"no confidence at all",
		"CONFIDENCE: 101",           // above the contracted range
		"CONFIDENCE: -1",            // below the contracted range
		"CONFIDENCE: 85.5",          // not an integer
		"CONFIDENCE: eighty-five",   // not a number at all
		"CONFIDENCE: 85%",           // trailing unit
		"CONFIDENCE: 85\nmore text", // not the LAST line
		"`CONFIDENCE: 85`",          // wrapped
		"confidence: 85",            // wrong case
		"VERDICT: APPROVE",          // the wrong contract entirely
	} {
		if score, ok := parseConfidenceScore(out); ok || score != 0 {
			t.Errorf("malformed/out-of-range confidence %q must yield (0,false); got (%v,%v)", out, score, ok)
		}
	}
}

// A claude JSON envelope whose `result` ends in a CONFIDENCE line must be UNWRAPPED
// first — the exact mirror of parseReviewerVerdict/parseExecutiveVerdict's envelope
// handling, proving parseConfidenceScore sees through the claude envelope the same way.
func TestParseConfidenceScore_UnwrapsClaudeEnvelope(t *testing.T) {
	envelope := `{"type":"result","total_cost_usd":0.01,"result":"需求分析完成...\nCONFIDENCE: 85"}`
	score, ok := parseConfidenceScore(envelope)
	if !ok || score != 85 {
		t.Errorf("a claude envelope's result must be unwrapped before scanning; got (%v,%v)", score, ok)
	}
}

// A trailing blank line after the CONFIDENCE line must be tolerated (the last
// NON-EMPTY line is what matters), the same trailing-blank tolerance
// parseReviewerVerdict already guarantees.
func TestParseConfidenceScore_TrailingBlankTolerated(t *testing.T) {
	score, ok := parseConfidenceScore("需求分析...\nCONFIDENCE: 72\n\n  ")
	if !ok || score != 72 {
		t.Errorf("trailing blank lines must not mask the confidence; got (%v,%v)", score, ok)
	}
}
