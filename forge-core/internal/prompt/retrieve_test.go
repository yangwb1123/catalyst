package prompt

import (
	"reflect"
	"testing"
)

// ids is a test helper: project the retrieved docs down to their IDs, which is
// what every ranking assertion actually cares about (order + membership).
func ids(docs []Doc) []string {
	out := make([]string, len(docs))
	for i, d := range docs {
		out[i] = d.ID
	}
	return out
}

// The core promise: a doc that shares the query's discriminating terms ranks
// above one that does not. This is the whole reason the retriever exists —
// "relevant top-K" must beat "first few".
func TestRetrieve_RelevantRanksAboveIrrelevant(t *testing.T) {
	docs := []Doc{
		{ID: "irrelevant", Text: "the build pipeline caches module downloads"},
		{ID: "relevant", Text: "ADR: prompt context retrieval ranks docs by relevance"},
		{ID: "tangential", Text: "logging format for the trace subsystem"},
	}
	got := ids(Retrieve(docs, "context retrieval relevance", 3))
	if len(got) == 0 || got[0] != "relevant" {
		t.Fatalf("expected 'relevant' ranked first, got order %v", got)
	}
	// The on-topic doc must outrank both off-topic docs, not merely appear.
	if indexOf(got, "relevant") >= indexOf(got, "irrelevant") {
		t.Errorf("'relevant' should outrank 'irrelevant'; order=%v", got)
	}
}

// k must cap the result to exactly the top-k by score (the context-budget guard).
func TestRetrieve_KTruncatesToTopScores(t *testing.T) {
	docs := []Doc{
		{ID: "a", Text: "alpha alpha alpha unique"}, // strongest on "alpha"
		{ID: "b", Text: "alpha beta"},
		{ID: "c", Text: "gamma delta"}, // no query-term overlap
	}
	got := ids(Retrieve(docs, "alpha", 2))
	if len(got) != 2 {
		t.Fatalf("k=2 must return 2 docs, got %d (%v)", len(got), got)
	}
	if got[0] != "a" {
		t.Errorf("top doc on 'alpha' should be 'a', got %v", got)
	}
	// 'c' shares no query term and must be excluded by the k=2 cutoff.
	if indexOf(got, "c") != -1 {
		t.Errorf("zero-overlap doc 'c' must not appear in top-2; got %v", got)
	}
}

// k beyond the corpus size returns everything, still relevance-ordered.
func TestRetrieve_KAboveLenReturnsAllRanked(t *testing.T) {
	docs := []Doc{
		{ID: "off", Text: "completely unrelated words"},
		{ID: "on", Text: "retrieval retrieval retrieval"},
	}
	got := ids(Retrieve(docs, "retrieval", 99))
	if len(got) != 2 {
		t.Fatalf("k>len must return all %d docs, got %d (%v)", len(docs), len(got), got)
	}
	if got[0] != "on" {
		t.Errorf("relevance order must hold even when k>len; got %v", got)
	}
}

// Fail-closed: k<=0 means "inject nothing", never "accidentally everything".
func TestRetrieve_NonPositiveKReturnsNil(t *testing.T) {
	docs := []Doc{{ID: "a", Text: "alpha"}, {ID: "b", Text: "beta"}}
	for _, k := range []int{0, -1, -100} {
		if got := Retrieve(docs, "alpha", k); got != nil {
			t.Errorf("k=%d must return nil, got %v", k, ids(got))
		}
	}
}

// Empty / whitespace / punctuation-only query has no tokens -> nothing to rank by,
// so inject nothing (defined and deterministic), rather than an arbitrary slice.
func TestRetrieve_EmptyQueryReturnsNil(t *testing.T) {
	docs := []Doc{{ID: "a", Text: "alpha"}, {ID: "b", Text: "beta"}}
	for _, q := range []string{"", "   ", "\t\n", "!!!  ---"} {
		if got := Retrieve(docs, q, 5); got != nil {
			t.Errorf("query %q has no tokens, must return nil, got %v", q, ids(got))
		}
	}
}

// Empty corpus -> empty result (no panic, no nonsense).
func TestRetrieve_EmptyDocsReturnsNil(t *testing.T) {
	if got := Retrieve(nil, "anything", 5); got != nil {
		t.Errorf("empty docs must return nil, got %v", ids(got))
	}
	if got := Retrieve([]Doc{}, "anything", 5); got != nil {
		t.Errorf("empty docs slice must return nil, got %v", ids(got))
	}
}

// Determinism on ties: when every doc scores identically (here, none shares the
// query term, so all score 0), the original input order must be preserved exactly
// — prompts have to be reproducible run to run.
func TestRetrieve_EqualScoresKeepInputOrder(t *testing.T) {
	docs := []Doc{
		{ID: "first", Text: "xxx"},
		{ID: "second", Text: "yyy"},
		{ID: "third", Text: "zzz"},
	}
	want := []string{"first", "second", "third"}
	// Run several times: a non-stable sort or map-iteration leak would flap here.
	for i := 0; i < 50; i++ {
		got := ids(Retrieve(docs, "no-overlap-term", 3))
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("tie ordering must equal input order %v, got %v (iter %d)", want, got, i)
		}
	}
}

// A common term shared by every doc is non-discriminating (idfWeight -> 0), so a
// doc that ALSO matches a rare query term must win. This pins the IDF-lite signal
// that makes "relevant" beat "shares only filler words".
func TestRetrieve_CommonTermDoesNotDominate(t *testing.T) {
	docs := []Doc{
		{ID: "filler", Text: "system system system system"},
		{ID: "rare", Text: "system orchestrator"}, // also has the rare term
	}
	// "system" is in both docs (df==total -> weight 0); "orchestrator" is rare.
	got := ids(Retrieve(docs, "system orchestrator", 2))
	if got[0] != "rare" {
		t.Errorf("doc matching the rare term should win over filler; got %v", got)
	}
}

// Length normalization: between two docs with the same single hit on the query
// term, the shorter (denser) doc is the stronger match and must rank first. The
// third, term-free doc is load-bearing: it keeps df("needle") < total so the
// IDF-lite weight stays non-zero — otherwise (term in every doc) the weight is 0
// and both scores collapse to a tie, masking the length signal under test.
func TestRetrieve_ShorterDenserDocWins(t *testing.T) {
	docs := []Doc{
		{ID: "long", Text: "needle buried among many many many other unrelated padding words here"},
		{ID: "short", Text: "needle here"},
		{ID: "none", Text: "totally different filler content with no hit"},
	}
	got := ids(Retrieve(docs, "needle", 3))
	if got[0] != "short" {
		t.Errorf("denser doc should rank first under length normalization; got %v", got)
	}
	if indexOf(got, "short") >= indexOf(got, "long") {
		t.Errorf("'short' (denser) must outrank 'long'; got %v", got)
	}
}

func TestTokenize(t *testing.T) {
	cases := []struct {
		name string
		in   string
		want []string // nil here asserts "no tokens" (len 0), regardless of nil-vs-empty.
	}{
		{"empty", "", nil},
		{"whitespace only", "   \t\n", nil},
		{"punctuation only", "!!!---,,,", nil},
		{"lowercases", "ADR Context", []string{"adr", "context"}},
		{"splits on punctuation", "fail-closed, top-K!", []string{"fail", "closed", "top", "k"}},
		{"keeps digits", "ADR 0001 v3", []string{"adr", "0001", "v3"}},
		{"collapses runs of separators", "a   b\t\tc", []string{"a", "b", "c"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := tokenize(tc.in)
			// For the no-token cases assert emptiness only: strings.FieldsFunc
			// idiomatically returns an empty (non-nil) slice, and the retriever
			// only ever checks len(tokens), so nil-vs-empty is not a contract.
			if len(tc.want) == 0 {
				if len(got) != 0 {
					t.Errorf("tokenize(%q) = %v, want no tokens", tc.in, got)
				}
				return
			}
			if !reflect.DeepEqual(got, tc.want) {
				t.Errorf("tokenize(%q) = %v, want %v", tc.in, got, tc.want)
			}
		})
	}
}

// indexOf returns the position of id in xs, or -1. Test-local helper.
func indexOf(xs []string, id string) int {
	for i, x := range xs {
		if x == id {
			return i
		}
	}
	return -1
}
