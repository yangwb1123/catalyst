package prompt

// This file is forge-core's Context/Memory retriever — the piece that lets the
// prompt carry *relevant* project context instead of *all* of it. Today Gather
// (prompt.go) injects everything: every ADR title plus the leading AGENTS.md
// bullets. That is fine for a young repo, but a real project accrues dozens of
// ADRs and pages of constraints, and fixed full injection would blow the model's
// context window. Retrieve scores candidate snippets against the current
// phase/task query and returns only the top-K — turning context injection from
// "everything, always" into "the relevant few".
//
// HONESTY — what this is and is not:
//   - This is v1: a pure keyword / term-frequency retriever, Go standard library
//     only, zero dependencies (strings/sort/unicode). It ranks by how often the
//     query's terms occur in a doc, down-weighting terms that are common across
//     the corpus (an IDF-lite signal) and normalizing by doc length so a long
//     doc cannot win on bulk alone. It is the BM25-lite family, not BM25 proper.
//   - It is NOT semantic. "car" will not match "vehicle"; only shared word stems
//     (lowercased, punctuation-split) count. True semantic / embedding retrieval
//     is v3 work: it needs vectorization and an external embedding model, neither
//     of which belongs in this zero-dependency runtime.
//   - Even when the corpus is small enough that top-K ≈ full injection, wiring
//     retrieval in now is the correct, scalable direction: the moment the repo
//     grows past a windowful of ADRs, the same call keeps the prompt bounded.
//
// Determinism is a hard requirement (prompts must be reproducible across runs):
// scoring uses no maps in the ordering path, and equal-scoring docs keep their
// original input order via a stable sort. Boundaries fail closed — k<=0 or no
// query yields an empty result rather than "accidentally everything".

import (
	"sort"
	"strings"
	"unicode"
)

// Doc is one retrievable context snippet: an ADR title, an AGENTS.md constraint,
// a code-symbol summary, etc. ID is an opaque caller-side handle (e.g. a file
// path or ADR number) used to trace what was injected; Text is the body that is
// both scored against the query and, ultimately, fed to the agent.
type Doc struct {
	ID   string
	Text string
}

// Retrieve returns the k docs most relevant to query, highest score first. It is
// a pure function: no I/O, no globals, deterministic for identical inputs.
//
// Contract / fail-closed boundaries:
//   - k <= 0: returns nil. "Give me zero or fewer" means zero, never the whole
//     corpus — that distinction is what stops a bad k from flooding the prompt.
//   - k >= len(docs): every doc is returned, still ranked by relevance.
//   - empty/whitespace query (no tokens): returns nil. With nothing to match on,
//     "most relevant" is undefined, so inject nothing rather than an arbitrary
//     slice. (Callers wanting unconditional context should use Gather, not this.)
//   - empty docs: returns nil.
//
// Ties (equal score) are broken by original input order via a stable sort, so the
// same docs+query always yield the same ordering.
//
// Zero-match query (non-empty, but no term hits any doc): every doc scores 0, so
// the stable sort leaves them in input order and the first k are returned — i.e. a
// no-hit query falls back to the top-k BY INPUT ORDER, not "the few relevant ones"
// (there are none). This is the v1 some-context-over-none default: a query that
// matches nothing still injects bounded context rather than dropping it entirely.
func Retrieve(docs []Doc, query string, k int) []Doc {
	if k <= 0 || len(docs) == 0 {
		return nil
	}
	qTerms := tokenize(query)
	if len(qTerms) == 0 {
		return nil // no query signal -> inject nothing (see contract above).
	}

	// Pre-tokenize every doc once; reuse for both the corpus stats (df) and the
	// per-doc scoring pass, so each doc is tokenized exactly once.
	docTerms := make([][]string, len(docs))
	df := make(map[string]int) // df[term] = number of docs that contain term.
	for i, d := range docs {
		toks := tokenize(d.Text)
		docTerms[i] = toks
		for term := range distinct(toks) {
			df[term]++
		}
	}

	// Score each doc, carrying its original index so equal scores stay stable.
	type scored struct {
		idx   int
		score float64
	}
	ranked := make([]scored, len(docs))
	for i := range docs {
		ranked[i] = scored{idx: i, score: score(qTerms, docTerms[i], df, len(docs))}
	}

	// Stable sort by descending score; SliceStable keeps original order on ties,
	// which (with the strictly-greater compare below) pins deterministic output.
	sort.SliceStable(ranked, func(a, b int) bool {
		return ranked[a].score > ranked[b].score
	})

	if k > len(docs) {
		k = len(docs)
	}
	out := make([]Doc, k)
	for i := 0; i < k; i++ {
		out[i] = docs[ranked[i].idx]
	}
	return out
}

// score rates one doc against the query terms (TF · IDF-lite, length-normalized).
//
// Why this shape:
//   - term frequency: a doc that mentions a query term more is more on-topic.
//   - IDF-lite (idfWeight): a term occurring in few docs is discriminating; a
//     term in every doc (e.g. a boilerplate word) carries little signal. This is
//     what lets "the relevant doc" beat one that merely shares filler words.
//   - length normalization: dividing by doc length stops a long doc from winning
//     on sheer volume of (possibly incidental) term hits.
//
// A doc sharing no query term scores 0; with an all-common query (every term in
// every doc) every weight is ~0, so all scores collapse to ~0 and the stable sort
// preserves input order — a defined, deterministic degenerate case.
func score(qTerms, docToks []string, df map[string]int, totalDocs int) float64 {
	if len(docToks) == 0 {
		return 0
	}
	tf := count(docToks)
	var sum float64
	for _, term := range qTerms {
		n, ok := tf[term]
		if !ok {
			continue
		}
		sum += float64(n) * idfWeight(df[term], totalDocs)
	}
	return sum / float64(len(docToks)) // normalize by doc length.
}

// idfWeight is an inverse-document-frequency-style weight without math/log (we
// keep to the named-as-allowed stdlib and avoid floating-point log noise): a term
// in fewer docs gets a larger weight. A term present in *every* doc (docFreq ==
// totalDocs) yields 0 — it cannot discriminate, so it must not sway ranking.
// Guarded so a never-seen term (docFreq 0) or empty corpus returns 0, not a
// divide-by-zero or spurious boost.
func idfWeight(docFreq, totalDocs int) float64 {
	if docFreq <= 0 || totalDocs <= 0 {
		return 0
	}
	// (total - df) / total: 1 when a term is rare (df=1, large corpus), 0 when it
	// is in every doc. Monotonic in rarity, bounded [0,1), no logs needed.
	return float64(totalDocs-docFreq) / float64(totalDocs)
}

// tokenize lowercases s and splits on any non-(letter|digit) rune, yielding the
// comparable terms used for both query and docs. Deliberately simple and
// deterministic: no stemming, no stopword list (a stopword that slipped through
// would still be neutralized by idfWeight, so the extra config buys nothing in
// v1). Empty / punctuation-only input yields nil.
func tokenize(s string) []string {
	return strings.FieldsFunc(strings.ToLower(s), func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r)
	})
}

// count tallies term frequency within a single doc's tokens.
func count(toks []string) map[string]int {
	m := make(map[string]int, len(toks))
	for _, t := range toks {
		m[t]++
	}
	return m
}

// distinct returns the set of unique terms in toks (used for document-frequency:
// a term counts once per doc no matter how often it repeats within that doc).
func distinct(toks []string) map[string]struct{} {
	set := make(map[string]struct{}, len(toks))
	for _, t := range toks {
		set[t] = struct{}{}
	}
	return set
}
