// prompt_memory.go — the cross-session MEMORY lane of the prompt's Context Engine,
// plus the sibling in-process LEDGERS that buildPrompt (prompt_context.go) reads
// state back from: phaseOutputLedger (feeds_forward output), verdictLedger (the
// reviewer's APPROVE/REQUEST_CHANGES), and reviewFindingsLedger (targeted-repair
// findings routed to the loop-back target). All are split out of prompt_context.go
// to keep both files under the volume budget (verdictLedger/reviewFindingsLedger
// were briefly their own prompt_verdict.go; merged back in here to keep cmd/forge's
// non-test file count under its package budget — the ledgers share truncateSummary
// and phaseOutputSummaryCap, so consolidating them removed a needless cross-file
// duplication risk too).
//
// memory's store is APPEND-ONLY: the evolve loop records one entry per iteration and
// never rewrites, so over a long run (the package doc's "a 24h run") it grows without
// bound. The old memoryContext injected EVERY entry, so the memory lane was the one
// prompt lane with no ceiling — on a marathon run it would eventually overrun the
// context window, the same "long-run inevitable" failure class as the 529 overload.
// This file bounds it: boundMemory caps the injected set with a recency-floor +
// relevance mix, reusing the internal/prompt BM25-lite Retrieve (the "everything → the
// relevant few" library) for the older entries while always keeping the freshest N so
// the latest lessons are never lost.
//
// Layering: selection lives HERE in cmd/forge (the prompt-building layer) and calls the
// prompt library DOWNWARD (prompt.Retrieve / prompt.Doc); internal/memory is untouched —
// it still only stores (append/load) and exposes its pure Query, with no notion of a cap.
package main

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"sync"

	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/prompt"
	"forgeos/forge-core/internal/tasklist"
)

// memoryCap is the most cross-session memory entries memoryContext will inject into
// one prompt — the budget that stops an ever-growing store from blowing the context
// window on a long run. The store is APPEND-ONLY (one entry per evolve iteration,
// never rewritten), so on a 24h / dozens-of-iterations run it grows without bound;
// injecting every entry (the old memoryContext did) would eventually overrun the
// window — the same "long-run inevitable" failure class as the 529 overload. 32 is
// chosen as comfortably above a normal store (so small/typical projects inject whole,
// byte-for-byte the prior behavior — existing memoryContext tests are unaffected) yet
// a hard ceiling so a marathon run stays bounded. It is the memory lane's twin of
// adrTopK / taskCap / phaseOutputSummaryCap — the last prompt lane that lacked a bound.
const memoryCap = 32

// memoryRecencyFloor is how many of the most RECENT entries (largest Iteration) are
// always injected when the store exceeds memoryCap, regardless of query relevance.
// It exists because memory's whole point is "do not relearn what you just learned":
// the freshest gaps/decisions/lessons must never be dropped. Pure relevance would not
// guarantee this — its zero-match fallback ranks by input (append) order, biasing
// OLD; this floor pins the newest N in so the latest lessons are never amnesiacally
// lost. The remaining (memoryCap - memoryRecencyFloor) slots go to the most RELEVANT
// older entries (so a still-pertinent old decision is kept, not torn down). 8 leaves
// a healthy 24 relevance slots under the cap.
const memoryRecencyFloor = 8

// Compile-time invariant: the recency floor must leave at least one relevance slot
// (0 < memoryRecencyFloor < memoryCap). boundMemory's "returns exactly memoryCap over
// the cap" guarantee depends on it — if the floor ever met or exceeded the cap, the
// relevance pass would get k<=0 and the floor alone could overshoot the cap. This makes
// a future edit that breaks the relation FAIL TO COMPILE rather than silently overflow
// the prompt budget. (A negative array length is a Go compile error.)
const _ = uint(memoryCap - memoryRecencyFloor - 1) // fails to compile if floor >= cap
const _ = uint(memoryRecencyFloor - 1)             // fails to compile if floor <= 0

// boundMemory bounds an unbounded entry slice to at most memoryCap entries, returned
// in Iteration-ASCENDING order (chronological, so the agent reads them as a coherent
// timeline rather than a relevance-jumbled list).
//
// Strategy and its HONEST trade-offs:
//   - len <= memoryCap: returned UNCHANGED, in input order — byte-for-byte the old
//     unbounded behavior for any normal store (this is what keeps existing tests/small
//     projects identical).
//   - len > memoryCap: a RECENCY FLOOR + RELEVANCE mix. The memoryRecencyFloor newest
//     entries are always kept (freshest lessons never lost); the rest of the budget is
//     filled by prompt.Retrieve's top-K against the phase query (a still-relevant OLD
//     decision survives). Floor ∪ relevance is de-duplicated, then sorted by Iteration.
//
// Trade-off (stated, not hidden): over the cap, a MIDDLE entry that is neither recent
// nor relevant to this phase is dropped — an old gap unrelated to the current work may
// be omitted. That is the necessary cost of not overrunning the window. Retrieval is
// v1 keyword BM25-lite (prompt.Retrieve), NOT semantic — "vehicle" won't match "car";
// the memory lane inherits that limit (semantic retrieval is v3). The recency floor
// guarantees the newest memoryRecencyFloor entries are always present regardless.
func boundMemory(entries []memory.Entry, query string) []memory.Entry {
	if len(entries) <= memoryCap {
		return entries
	}
	keep := recentFloorSet(entries, memoryRecencyFloor)
	for _, e := range relevantOlder(entries, query, memoryCap-len(keep), keep) {
		keep[e] = struct{}{}
	}
	out := make([]memory.Entry, 0, len(keep))
	for i, e := range entries {
		if _, ok := keep[i]; ok {
			out = append(out, e)
		}
	}
	sort.SliceStable(out, func(a, b int) bool { return out[a].Iteration < out[b].Iteration })
	return out
}

// recentFloorSet returns the indices of the n entries with the largest Iteration — the
// always-in recency floor. The sort is STABLE on a descending-Iteration comparator, so
// among entries that TIE on Iteration the EARLIER input position is kept first (stable
// sort preserves input order on equal keys); in practice the evolve log's Iterations are
// distinct and monotonic, so ties do not arise. n is clamped to len(entries). Indices
// (not entries) are returned so the caller can de-dup against the relevance pick by
// original position.
func recentFloorSet(entries []memory.Entry, n int) map[int]struct{} {
	idx := make([]int, len(entries))
	for i := range entries {
		idx[i] = i
	}
	sort.SliceStable(idx, func(a, b int) bool { return entries[idx[a]].Iteration > entries[idx[b]].Iteration })
	if n > len(idx) {
		n = len(idx)
	}
	set := make(map[int]struct{}, n)
	for _, i := range idx[:n] {
		set[i] = struct{}{}
	}
	return set
}

// relevantOlder selects up to k indices of entries (excluding those already in `have`)
// most relevant to query via prompt.Retrieve — the BM25-lite library built for exactly
// this "everything → the relevant few" job. Each candidate becomes a prompt.Doc whose ID
// is its ORIGINAL index (so the ranked result maps straight back to entries). k<=0 or no
// candidates yields nil. Honest: keyword retrieval, not semantic (see boundMemory).
func relevantOlder(entries []memory.Entry, query string, k int, have map[int]struct{}) []int {
	if k <= 0 {
		return nil
	}
	var docs []prompt.Doc
	for i, e := range entries {
		if _, taken := have[i]; taken {
			continue
		}
		docs = append(docs, prompt.Doc{ID: strconv.Itoa(i), Text: e.Kind + " " + e.Topic + " " + e.Detail})
	}
	var out []int
	for _, d := range prompt.Retrieve(docs, query, k) {
		if i, err := strconv.Atoi(d.ID); err == nil {
			out = append(out, i)
		}
	}
	return out
}

// memoryContext renders the cross-session store as one BOUNDED context block so the
// agent sees what prior iterations learned WITHOUT the store (which grows every evolve
// iteration, unbounded over a long run) eventually blowing the context window. Selection
// is boundMemory's recency-floor + relevance mix keyed off `query` (the phase identity,
// aligned with Gather) — a change from the old "inject every entry" behavior. The honest
// trade-offs (a non-recent/non-relevant middle entry may be dropped over the cap; keyword
// not semantic retrieval; newest memoryRecencyFloor always kept) live on boundMemory.
// Below the cap, every entry is still injected in input order, byte-for-byte as before.
// Missing store = cold start (no block, no error); a malformed store is surfaced as a
// visible context line, not an aborted prompt.
func memoryContext(repoRoot, query string) []string {
	entries, err := memory.Load(memoryPath(repoRoot))
	if err != nil {
		return []string{"Project memory: UNREADABLE (" + err.Error() + ")"}
	}
	rel := boundMemory(entries, query)
	if len(rel) == 0 {
		return nil
	}
	var b strings.Builder
	b.WriteString("Project memory (gaps / decisions / lessons from prior iterations):")
	for _, e := range rel {
		prefix := ""
		if e.Confidence < 0.3 {
			prefix = " [unverified]"
		} else if e.Confidence < 0.7 {
			prefix = " [low-confidence]"
		}
		fmt.Fprintf(&b, "\n- [%s]%s %s — %s (iter %d)", e.Kind, prefix, e.Topic, e.Detail, e.Iteration)
		if e.Source != "" {
			fmt.Fprintf(&b, " [source: %s]", e.Source)
		}
	}
	return []string{b.String()}
}

// phaseOutputSummaryCap bounds how many bytes of a fed-forward phase's output are
// remembered. The planner's output (a sprint split + acceptance criteria) can be long;
// injecting it whole into every later phase's prompt would bloat the prompt (and the
// token bill) without adding signal past the first screenful. record truncates beyond
// this cap and appends an ellipsis marker so the downstream agent knows it was clipped.
// Shared by phaseOutputLedger and reviewFindingsLedger below.
const phaseOutputSummaryCap = 800

// truncateSummary clips s to phaseOutputSummaryCap runes, appending a marker when it
// did clip so the reader knows the output continued. Rune-based (not byte-based) so a
// multi-byte UTF-8 boundary is never split mid-character.
func truncateSummary(s string) string {
	r := []rune(s)
	if len(r) <= phaseOutputSummaryCap {
		return s
	}
	return string(r[:phaseOutputSummaryCap]) + " …(已截断)"
}

// phaseOutputLedger accumulates the output of every phase whose asset declares
// feeds_forward, keyed by phase name, preserving first-seen order for a stable render.
// It is the in-memory bridge between the agent executor's Observe sink (which writes it,
// one phase at a time, for a feeds_forward phase only) and buildPrompt (which reads
// contextLines() into a LATER phase's prompt) — the EXACT structural mirror of
// gateLedger (prompt_context.go), but carrying planner-style planning output instead of
// gate verdicts.
//
// CONCURRENCY: mu guards summary+order for the OPT-IN parallel orchestrator (the
// parallelize-then-lock the prior comment foresaw); the serial path takes the
// uncontended lock and is byte-for-byte unchanged.
type phaseOutputLedger struct {
	mu      sync.Mutex        // guards summary+order under parallel execution
	summary map[string]string // phase name -> latest (truncated) output summary
	order   []string          // phase names in first-seen order (stable rendering)
	plans   map[string]tasklist.Plan
}

// newPhaseOutputLedger returns an empty ledger ready to record phase outputs.
func newPhaseOutputLedger() *phaseOutputLedger {
	return &phaseOutputLedger{summary: map[string]string{}, plans: map[string]tasklist.Plan{}}
}

// record stores one phase's latest output, truncated to phaseOutputSummaryCap.
func (l *phaseOutputLedger) record(phase, output string) {
	l.recordWithPolicy(phase, output, false)
}

// recordExact stores a machine-validated, independently bounded output without
// applying the ordinary summary truncation. The evolve_scan_v1 adapter calls this
// only after strict decoding and canonicalization.
func (l *phaseOutputLedger) recordExact(phase, output string) {
	l.recordWithPolicy(phase, output, true)
}

func (l *phaseOutputLedger) recordWithPolicy(phase, output string, exact bool) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if _, seen := l.summary[phase]; !seen {
		l.order = append(l.order, phase)
	}
	if strings.Contains(output, "TASK_LIST:") {
		if plan, err := tasklist.Parse(output); err == nil {
			l.plans[phase] = plan
			output = tasklist.Render(plan)
		}
	}
	if exact {
		l.summary[phase] = output
		return
	}
	l.summary[phase] = truncateSummary(output)
}

func (l *phaseOutputLedger) output(phase string) (string, bool) {
	if l == nil {
		return "", false
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	output, ok := l.summary[phase]
	return output, ok
}

// recommendedTaskModel returns the first dependency-ready planner task's model
// hint. The parser stores plans in stable topological order, so index zero is
// always executable without an unmet task dependency. Empty/unparsed plans have
// no hint and leave routing unchanged.
func (l *phaseOutputLedger) recommendedTaskModel() string {
	if l == nil {
		return ""
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	for _, phase := range l.order {
		if plan, ok := l.plans[phase]; ok && len(plan.Tasks) > 0 {
			return plan.Tasks[0].Model
		}
	}
	return ""
}

// context renders the recorded phase outputs as a prompt context block, or "" when the
// ledger is nil or empty (no feeds_forward phase has run yet). The text is HONEST about
// what this is: the prior PLANNING phase's own output (the planner's task split /
// acceptance criteria), offered to the implementer and reviewer as reference — NOT a
// gate verdict, NOT a fabricated fact. Each phase renders as a labeled block in
// first-seen order.
func (l *phaseOutputLedger) context() string {
	if l == nil {
		return ""
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if len(l.order) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString("前序声明前传阶段产出(包括 planner 的任务拆分/验收标准或机器校验报告,供后续阶段参考 —— 这是上游阶段产出,不是闸门结果):")
	for _, name := range l.order {
		b.WriteString("\n\n### ")
		b.WriteString(name)
		b.WriteString("\n")
		b.WriteString(l.summary[name])
	}
	return b.String()
}

// contextLines wraps context() as a slice ready to append onto a prompt's context lanes
// (nil when there is nothing to inject — symmetric with gateLedger.contextLines and
// memoryContext), so buildPrompt stays a flat sequence of appends with no inline check.
func (l *phaseOutputLedger) contextLines() []string {
	if c := l.context(); c != "" {
		return []string{c}
	}
	return nil
}

// verdictLedger remembers the latest machine-readable verdict of every AGENT phase
// that emitted one (today: the reviewer's APPROVE/REQUEST_CHANGES), keyed by phase
// name. It is the cmd/forge end of Engine.AgentVerdict's PULL wire — the reverse twin
// of gateLedger (which serves the OnGateResult PUSH): the Observe sink writes it via
// record, and the orchestrator reads it back via get. Storing the NORMALIZED token
// (not the raw output) keeps the engine vendor-free.
//
// CONCURRENCY: mu guards verdict for the OPT-IN parallel orchestrator (uncontended,
// byte-for-byte unchanged on the serial path).
type verdictLedger struct {
	mu      sync.Mutex
	verdict map[string]string // phase name -> latest normalized verdict token
}

// newVerdictLedger returns an empty ledger ready to record agent verdicts.
func newVerdictLedger() *verdictLedger { return &verdictLedger{verdict: map[string]string{}} }

// record stores one phase's latest verdict (the normalized APPROVE/REQUEST_CHANGES
// token), overwriting a prior one so a re-run reviewer's NEWEST verdict wins. Safe on a
// nil receiver (no-op) so a run that never built a ledger cannot panic.
func (l *verdictLedger) record(phase, verdict string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.verdict[phase] = verdict
}

// clear starts a new execution attempt for phase. Observe sees provider output
// even when a command later fails its exit/output contract (cost telemetry still
// needs those bytes), so Build clears any verdict from the previous attempt
// before the next spawn. A failed APPROVE can therefore never leak into a later
// successful-but-malformed retry.
func (l *verdictLedger) clear(phase string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	delete(l.verdict, phase)
}

// get reads back a phase's recorded verdict for Engine.AgentVerdict: (token, true) when
// one was recorded, ("", false) when none was — nil-safe, so an unwired ledger reports
// "no verdict"; the orchestrator then applies the phase's advisory or required posture.
func (l *verdictLedger) get(phase string) (string, bool) {
	if l == nil {
		return "", false
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	v, ok := l.verdict[phase]
	return v, ok
}

// wasReworked returns true when any phase recorded a REQUEST_CHANGES verdict —
// the "rework" signal for trajectory-aware scorecard updates and the Reflect step.
// Nil-safe (no verdicts = no rework). Used by the wind-down to carry the real
// reviewer-bounce signal into the learning loop's trajectory row.
func (l *verdictLedger) wasReworked() bool {
	if l == nil {
		return false
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	for _, v := range l.verdict {
		switch v {
		case VerdictRequestChanges, VerdictRedesign, VerdictDelay, VerdictReject:
			return true
		}
	}
	return false
}

// reviewFindingsLedger carries upstream review/QA findings BACKWARD across a directed
// loop-back, to be injected into the IMPLEMENTER's next prompt for targeted repair. It
// is a deliberately ONE-DIRECTION edge: keyed by the loop-back TARGET phase (the
// implementer, read from the reviewer phase's on_fail.target — data-driven, zero
// hard-coded agent name), and buildPrompt injects it ONLY into that target phase. The
// source phase, when it re-runs, has p.Name != target, so it NEVER receives these findings —
// preserving fresh-context reviewer independence (the D3/AGENTS red line).
//
// CONCURRENCY: mu guards findings for the OPT-IN parallel orchestrator, as the sibling
// ledgers do (uncontended on the serial path).
type reviewFindingsLedger struct {
	mu       sync.Mutex
	findings map[string]string // loop-back TARGET phase -> latest (truncated) findings
}

// newReviewFindingsLedger returns an empty ledger ready to record repair findings.
func newReviewFindingsLedger() *reviewFindingsLedger {
	return &reviewFindingsLedger{findings: map[string]string{}}
}

// record stores upstream findings for the loop-back TARGET phase (its recipient),
// TRUNCATED to phaseOutputSummaryCap so a verbose review cannot bloat the implementer's
// prompt — reusing the same cap/marker as phaseOutputLedger. A re-review overwrites with
// the newest findings. Safe on a nil receiver (no-op).
func (l *reviewFindingsLedger) record(targetPhase, findings string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.findings[targetPhase] = truncateSummary(findings)
}

// contextLines renders the findings recorded for the phase named `name` as a single
// appendable prompt block, or nil when none were recorded for it (the common case — the
// gate is in buildPrompt, which only consults this for the implementer). The text is
// HONEST about provenance without inventing one source: Reviewer, QA and executive
// review can all feed this lane, and the original evidence retains its own verdict word.
func (l *reviewFindingsLedger) contextLines(name string) []string {
	if l == nil {
		return nil
	}
	l.mu.Lock()
	f, ok := l.findings[name]
	l.mu.Unlock()
	if !ok || f == "" {
		return nil
	}
	return []string{"上一轮上游审查/验收角色给出的逐条修复证据(供定向修复参考;非闸门结果,裁决词以原始证据为准):\n\n" + f}
}

// allFindings returns a snapshot copy of all recorded (targetPhase→findings) entries
// for the Reflect step: structured memory extraction after an iteration completes.
// Returns nil when the ledger is empty or nil. The copy lets callers safely read
// after the ledger's next write proceeds.
func (l *reviewFindingsLedger) allFindings() map[string]string {
	if l == nil {
		return nil
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if len(l.findings) == 0 {
		return nil
	}
	out := make(map[string]string, len(l.findings))
	for k, v := range l.findings {
		out[k] = v
	}
	return out
}
