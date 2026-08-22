// gates.go — the honest bridge from a workflow's REQUIRED gate names to their
// real per-gate verdicts. This is the fix for the FAKE PASS (FC-01): instead of
// collapsing lint/build/security onto one coarse aggregate result, each gate
// resolves to its OWN tri-state (PASS/FAIL/NA) using a shared fresh acceptance
// snapshot, and "gates green" means every required gate was actually CHECKED
// and PASSED — never merely "nothing failed".
//
// The N/A exemption matrix and per-gate resolution (GatesGreen/ResolveGate/…)
// live in internal/gate (resolve.go) — pure business logic, not CLI glue, so
// it belongs in the package that already owns gate.Result/ProbeAll rather than
// in cmd/forge (see internal/attribution for the same precedent). The
// approve-list CLI lives in approve.go; this file keeps higher-level orchestration:
// gatherSignals (the Signals{} builder every stop condition is judged
// against), the per-iteration probe cache, and the honesty-enrichment
// computers (CodeTestRatio, FileDelta).
package main

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"unicode"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/statefs"
)

// gatherSignals measures the live convergence inputs: the fraction of ROADMAP
// checklist items done, and whether EVERY required gate of the workflow's gate
// phases actually resolved to PASS. GatesGreen is true only when no required
// gate is FAIL and none is N/A — "green" means every required gate was CHECKED
// and PASSED, never merely "nothing failed".
//
// ctx/opts bound the LIVE gate spawns the convergence check can trigger
// (complexity/arch via gate.GatesGreenWith). The same fresh acceptance map
// (criterion -> PASS/FAIL/NA) also feeds Signals.Criteria (a workflow can
// converge on an INDIVIDUAL criterion too); a nil probe leaves Criteria nil and
// every per-criterion check unmet. verdicts (nil-safe) drives ReviewStatus via
// reviewStatus(); gateSet optionally supplies the exact mode-filtered gates the
// engine executed (else the workflow-declared fallback).
func gatherSignals(ctx context.Context, opts gate.Options, root string, wf asset.Workflow, probe, categories map[string]string, lifecycle string, approved bool, verdicts *verdictLedger, gateSet ...[]string) converge.Signals {
	md, _ := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	names := requiredGates(wf)
	if len(gateSet) > 0 {
		names = gateSet[0]
	}
	green, proof := gate.GatesGreenWith(ctx, root, names, probe, categories, lifecycle, opts)
	sig := converge.Signals{
		RoadmapCompletion:     converge.RoadmapCompletion(string(md)),
		GatesGreen:            green,
		GateProof:             proof,
		Criteria:              probe,
		HumanApproved:         approved,
		CodeTestRatio:         computeCodeTestRatio(root),
		ReviewStatus:          reviewStatus(verdicts),
		RequirementConfidence: requirementConfidence(wf, verdicts),
		FileDelta:             computeFileDelta(root),
	}
	// Warn when prod code changes without corresponding test code (analysis §5.2).
	if sig.CodeTestRatio == 0 && sig.RoadmapCompletion > 0 {
		fmt.Printf("forge: test-gap warning — changed lines are 100%% production code with 0%% tests (CodeTestRatio=0); consider adding tests for new code\n")
	}
	return sig
}

// executiveReviewPhase is review.yml P4's fixed phase name (agent: cto), the same
// literal converge.go's evalReviewStatus implicitly targets via "review_status".
const executiveReviewPhase = "executive-review"

// reviewStatus resolves Signals.ReviewStatus from the CTO's recorded executive-review
// verdict (cost.go's parseExecutiveVerdict, wired via prompt_context.go's observeFor)
// — the previously-missing wire that left ReviewStatus permanently "". Nil-safe
// (verdictLedger.get tolerates nil), so an unwired/not-yet-run phase stays "" (honest
// absence, never a fabricated approval). APPROVE/APPROVE_WITH_SIMPLIFICATION -> "approved";
// REDESIGN/DELAY/REJECT -> their own lowercase token (a MEANINGFUL unmet detail, not a
// blank "no review phase data"); anything else (not yet recorded) -> "".
func reviewStatus(verdicts *verdictLedger) string {
	switch v, _ := verdicts.get(executiveReviewPhase); v {
	case VerdictApprove, VerdictApproveWithSimplification:
		return "approved"
	case VerdictRedesign:
		return "redesign"
	case VerdictDelay:
		return "delay"
	case VerdictReject:
		return "reject"
	default:
		return ""
	}
}

// requirementDiscoveryPhase is the FALLBACK phase name consulted when no phase
// in the workflow declares confidence_metric: requirement_confidence — discover.yml
// P1's fixed phase name (agent: product-manager) today, the same literal
// converge.go's evalRequirementConfidence implicitly targets via
// "requirement_confidence", and the numeric-signal counterpart to
// executiveReviewPhase. Preserved as the default so every workflow authored
// before confidenceMetricPhase existed keeps its exact current behavior.
const requirementDiscoveryPhase = "requirement-discovery"

// requirementConfidenceMetric is the confidence_metric value discover.yml's
// requirement-discovery phase declares — the field confidenceMetricPhase scans
// wf.Phases for.
const requirementConfidenceMetric = "requirement_confidence"

// confidenceMetricPhase is the field-driven counterpart to mode_gating.go's
// requiredWhenKey-style dispatch: instead of a hardcoded phase name, it scans
// wf.Phases for the phase that declares `confidence_metric: <metric>` and
// returns THAT phase's Name, so a caller can look up its recorded verdict
// regardless of what the phase is called. Falls back to
// requirementDiscoveryPhase when no phase in the workflow declares the field —
// the byte-for-byte back-compat path, since discover.yml's own phase is in
// fact named "requirement-discovery" today.
func confidenceMetricPhase(wf asset.Workflow, metric string) string {
	for _, p := range wf.Phases {
		if p.ConfidenceMetric == metric {
			return p.Name
		}
	}
	return requirementDiscoveryPhase
}

// requirementConfidence resolves Signals.RequirementConfidence from the
// recorded confidence score of WHICHEVER phase in wf declares
// confidence_metric: requirement_confidence (confidenceMetricPhase — falling
// back to the literal requirementDiscoveryPhase when no phase declares it,
// e.g. discover.yml's product-manager requirement-discovery phase today; cost.go's
// parseConfidenceScore, wired via prompt_context.go's observeFor as a THIRD
// fallback verdict tier) — mirrors reviewStatus's structure exactly, but the
// stored payload is a numeric string ("85"), not a fixed token, so this reads
// it back and parses it. Nil-safe (verdictLedger.get tolerates nil), so an
// unwired/not-yet-run phase — or a recorded value that somehow fails to parse
// (should not happen, since only parseConfidenceScore ever writes to this
// phase's slot, but handled defensively rather than assumed) — stays 0
// (honest absence, never a fabricated confidence).
func requirementConfidence(wf asset.Workflow, verdicts *verdictLedger) float64 {
	phase := confidenceMetricPhase(wf, requirementConfidenceMetric)
	v, ok := verdicts.get(phase)
	if !ok {
		return 0
	}
	n, err := strconv.ParseFloat(v, 64)
	if err != nil {
		return 0
	}
	return n
}

// approvalPath is the on-disk human-approval marker for a stage: its mere
// EXISTENCE under <root>/.forge/<stage>.approved is one of the two approval
// signal sources (the other is the --approved flag). It lives in the git-ignored
// .forge runtime dir, so an approval is a deliberate local act, never committed.
func approvalPath(root, stage string) string {
	return filepath.Join(forgeDir(root), stage+".approved")
}

// rejectionPath is the on-disk human-REJECTION marker for a stage: its mere
// EXISTENCE under <root>/.forge/<stage>.rejected is the rejection SIGNAL,
// mirroring approvalPath exactly (same git-ignored .forge runtime dir; a
// rejection is a deliberate local act, never committed). Unlike approval there
// is no --rejected flag: the marker is the durable local decision, retained
// across failed rework and consumed after successful rework.
func rejectionPath(root, stage string) string {
	return filepath.Join(forgeDir(root), stage+".rejected")
}

// rejectionPhaseIndex is the cmd/forge-local counterpart to orchestrator's
// unexported phaseIndex (internal/orchestrator/orchestrator.go): the identical
// by-name phase lookup, duplicated here rather than exported because it is five
// lines and pulling a whole export across the package boundary for that would
// be the heavier change. Returns ok=false when no phase carries that name (an
// unresolvable on_rejected.target_phase).
func rejectionPhaseIndex(wf asset.Workflow, name string) (int, bool) {
	for i, p := range wf.Phases {
		if p.Name == name {
			return i, true
		}
	}
	return 0, false
}

// resolveRejectionStartPhase is `forge run`'s SINGLE-PASS counterpart to
// LoopEngine.nextStartPhase's OnRejected branch (internal/orchestrator/loop.go)
// — the missing call site that made design.yml's on_rejected unreachable (see
// that function's doc comment: `forge run` never loops, `forge evolve` refuses
// human_gate workflows outright). This is invoked ONCE, BEFORE runWorkflow, and
// resolves where THIS run should start:
//
//   - no on_rejected, or its action isn't "loop_back" -> phase 0 (nothing to
//     act on; a marker, if any, is left alone since it was never consumed).
//   - marker absent -> phase 0 (no rejection was filed; back-compat default).
//   - marker present, on_rejected actionable, target_phase resolves -> that
//     phase's index and rejected=true. The marker remains durable until the
//     caller confirms the resulting workflow completed successfully.
//   - marker present but target_phase does not resolve to a real phase ->
//     phase 0, marker left in place (nothing was acted on, so nothing is
//     consumed — an operator can fix the workflow's target_phase and re-run).
//
// Workflows without an actionable marker keep the phase-0 default.
func resolveRejectionStartPhase(wf asset.Workflow, root string, logln func(string)) (int, bool, error) {
	if wf.Stop.OnRejected == nil || wf.Stop.OnRejected.Action != "loop_back" {
		return 0, false, nil
	}
	rp := rejectionPath(root, wf.Stage)
	present, err := markerExists(rp)
	if err != nil {
		return 0, false, fmt.Errorf("inspect rejection marker %s: %w", rp, err)
	}
	if !present {
		return 0, false, nil
	}
	idx, ok := rejectionPhaseIndex(wf, wf.Stop.OnRejected.TargetPhase)
	if !ok {
		if logln != nil {
			logln(fmt.Sprintf("forge run: REJECTED marker found but on_rejected.target_phase %q not found — starting at phase 0 (marker left in place)", wf.Stop.OnRejected.TargetPhase))
		}
		return 0, false, nil
	}
	if logln != nil {
		logln(fmt.Sprintf("forge run: REJECTED (marker retained until successful rework) — resuming at phase %d (%s) per on_rejected.target_phase",
			idx, wf.Stop.OnRejected.TargetPhase))
	}
	return idx, true, nil
}

func consumeRejectionAfterSuccess(wf asset.Workflow, root string, rejected bool, logln func(string)) error {
	if !rejected {
		return nil
	}
	path := rejectionPath(root, wf.Stage)
	if err := statefs.RemoveRegular(path); err != nil {
		return fmt.Errorf("consume rejection marker %s after successful rework: %w", path, err)
	}
	if logln != nil {
		logln(fmt.Sprintf("forge run: REJECTED marker consumed after successful %s rework", wf.Stage))
	}
	return nil
}

// requiredGates collects the de-duplicated set of gate names across the
// workflow's gate phases — the gates whose collective PASS defines "green".
func requiredGates(wf asset.Workflow) []string {
	seen := map[string]bool{}
	var names []string
	for _, p := range wf.Phases {
		for _, g := range p.RequiredGates {
			if !seen[g] {
				seen[g] = true
				names = append(names, g)
			}
		}
	}
	return names
}

// loopProbe caches one acceptance probe per loop iteration. The repo changes
// between iterations (agents edit code), so the probe must be re-run each
// iteration — but the gate phases and the convergence check WITHIN an iteration
// must see the SAME probe. refresh() re-probes only when the cache is stale
// (first gate call of an iteration) and returns the status map; current() returns
// BOTH the status and category maps and marks the cache stale so the next
// iteration re-probes. This avoids double-spawning within an iteration while
// staying fresh across iterations, and keeps statuses+categories from the SAME run.
// ctx/opts ride the probe so every live spawn is bounded and Ctrl-C reaches the
// process group (A1.2 closure capture).
type loopProbe struct {
	// mu guards the cache: concurrent gate phases under `forge evolve --parallel`
	// (≥2 in one wave) prime it exactly once instead of racing; current() locks too.
	mu         sync.Mutex
	ctx        context.Context
	root       string
	opts       gate.Options
	cached     map[string]string
	categories map[string]string
	primed     bool
}

// refresh runs gate.ProbeAll once per iteration (caching both the status and
// category maps) and returns the status map — the gate-resolution view, which
// only needs statuses. A probe error degrades both maps to nil (downstream
// treats absent criteria as N/A with an empty, non-exemptible category).
func (p *loopProbe) refresh() map[string]string {
	p.mu.Lock()
	defer p.mu.Unlock()
	if !p.primed {
		statuses, categories, err := gate.ProbeAllWith(p.ctx, p.root, p.opts)
		if err != nil {
			fmt.Fprintf(os.Stderr, "forge evolve: acceptance probe unavailable (%v); gates degrade to N/A\n", err)
			statuses, categories = nil, nil
		}
		p.cached = statuses
		p.categories = categories
		p.primed = true
	}
	return p.cached
}

// current returns the iteration's probe statuses AND categories (priming them if
// the gate phases somehow did not), then marks the cache stale so the next
// iteration re-probes the changed repo. The convergence check needs both maps.
func (p *loopProbe) current() (statuses, categories map[string]string) {
	p.refresh() // locks internally to prime once
	p.mu.Lock()
	s, c := p.cached, p.categories
	p.primed = false
	p.mu.Unlock()
	return s, c
}

// isTestPath reports whether a git-diff-reported path names a test file:
// Go's *_test.go (Contains "_test") or Python's test_*.py (HasPrefix
// "test_"). The test_* check runs against the BASENAME, not the full path —
// "tests/test_foo.py" (pytest's dominant layout) starts with "tests/", not
// "test_", so checking the full path silently missed non-root test files.
func isTestPath(path string) bool {
	return strings.Contains(path, "_test") || strings.HasPrefix(filepath.Base(path), "test_")
}

// computeCodeTestRatio runs `git diff --stat HEAD` and returns the fraction of
// changed lines that are in test files (test = file names containing _test or
// starting with test_). 0 means either no changes or 100% production code.
// Returns 0 on any error (no git, no diff, or parse failure) — enrichment,
// never a convergence input (analysis §5.2).
func computeCodeTestRatio(root string) float64 {
	out, err := exec.Command("git", "-C", root, "diff", "--stat", "HEAD").Output()
	if err != nil {
		return 0
	}
	var prodLines, testLines int
	for _, line := range strings.Split(string(out), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || !strings.Contains(line, "|") {
			continue
		}
		// Parse: "path/to/file.go | 42 ++++++++++++++++++"
		parts := strings.SplitN(line, "|", 2)
		if len(parts) != 2 {
			continue
		}
		path := strings.TrimSpace(parts[0])
		// Extract the number before the first space after |
		numStr := strings.Fields(strings.TrimSpace(parts[1]))
		if len(numStr) == 0 {
			continue
		}
		n := 0
		if _, err := fmt.Sscanf(numStr[0], "%d", &n); err != nil || n == 0 {
			continue
		}
		if isTestPath(path) {
			testLines += n
		} else {
			prodLines += n
		}
	}
	total := prodLines + testLines
	if total == 0 {
		return 0
	}
	return float64(testLines) / float64(total)
}

// computeFileDelta measures the fraction of DONE roadmap items (checklist lines
// "- [x]"/"- [X]") that have at least one corresponding file-system change in the
// current git diff — Signals.FileDelta's honesty cross-check (converge.go's doc
// comment: "roadmap items that have corresponding file-system changes"), consumed
// by orchestrator/loop.go's reportConvergence to flag a claimed-100%-but-nothing-
// changed gap.
// DESIGN CHOICE — DONE items only, not pending/partial: the honesty question this
// answers is "if the agent claims something is done, is there file evidence for
// it?" A pending ("- [ ]") or partial ("- [~]") item carries no completion claim
// to cross-check yet, so folding it into the denominator would only dilute the
// signal with items nobody asserted were finished — the more faithful reading of
// the doc comment, and the one that actually detects the self-report-overstates-
// progress failure mode the caller warns about.
// This is a CHEAP HEURISTIC PROXY, not a precise link — the exact same honesty
// posture as internal/risk.FromChangedPaths: a keyword substring match is neither
// necessary nor sufficient evidence a roadmap item was truly implemented (a
// coincidental word match counts as a "hit"; a real implementation phrased with
// different vocabulary is missed as a "miss"). It exists to catch the EGREGIOUS
// case (high completion claimed, near-zero files touched), not to audit whether
// any individual item was correctly implemented.
//
// Returns 0 on any error (missing ROADMAP.md, no git, no diff) or when there are
// no DONE items to check — enrichment only, never a convergence input, mirroring
// computeCodeTestRatio's error posture exactly.
func computeFileDelta(root string) float64 {
	md, err := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	if err != nil {
		return 0
	}
	items := doneRoadmapItems(string(md))
	if len(items) == 0 {
		return 0
	}
	// --name-only (not computeCodeTestRatio's --stat): FileDelta only needs the
	// changed PATHS to keyword-match against, not line counts. Same base ref
	// (HEAD) as computeCodeTestRatio, for consistency.
	out, err := exec.Command("git", "-C", root, "diff", "--name-only", "HEAD").Output()
	if err != nil {
		return 0
	}
	changed := strings.Fields(string(out))
	matched := 0
	for _, item := range items {
		if itemTouchesAnyPath(item, changed) {
			matched++
		}
	}
	return float64(matched) / float64(len(items))
}

// doneRoadmapItems extracts the item TEXT (the substring after the checkbox
// marker) of every DONE ("- [x]"/"- [X]") checklist line in a ROADMAP markdown —
// the same line-matching convention as converge.RoadmapCompletion (kept
// independent rather than shared, since that function only needs a count, not
// the text) — restricted to done items per computeFileDelta's design choice above.
func doneRoadmapItems(markdown string) []string {
	var items []string
	for _, line := range strings.Split(markdown, "\n") {
		t := strings.TrimSpace(line)
		switch {
		case strings.HasPrefix(t, "- [x]"):
			items = append(items, strings.TrimSpace(t[len("- [x]"):]))
		case strings.HasPrefix(t, "- [X]"):
			items = append(items, strings.TrimSpace(t[len("- [X]"):]))
		}
	}
	return items
}

// itemTouchesAnyPath reports whether any MEANINGFUL keyword extracted from a
// roadmap item's text appears as a case-insensitive substring of any changed
// file path — the cheap heuristic match itself; see computeFileDelta's doc
// comment for its honesty posture (a proxy, not proof).
func itemTouchesAnyPath(item string, changedPaths []string) bool {
	for _, kw := range itemKeywords(item) {
		for _, path := range changedPaths {
			if strings.Contains(strings.ToLower(path), kw) {
				return true
			}
		}
	}
	return false
}

// fileDeltaStopWords are short/common words filtered out of a roadmap item's
// text before keyword-matching, because they would otherwise match almost any
// changed path and produce a meaningless "hit" (the same reason
// internal/risk.FromChangedPaths uses a fixed needle table rather than raw
// words) — here derived from prose instead of a fixed surface list, so the
// filter targets English filler rather than a domain vocabulary.
var fileDeltaStopWords = map[string]bool{
	"this": true, "that": true, "with": true, "from": true, "into": true,
	"then": true, "than": true, "have": true, "will": true, "when": true,
	"were": true, "been": true, "does": true, "each": true, "such": true,
}

// itemKeywords extracts lowercase, meaningful (>=4 rune) words from a roadmap
// item's text, splitting on anything that is not a letter or digit (so
// backtick-quoted paths like `harness/gate.mjs` yield "harness"/"gate"/"mjs"
// as separate candidate tokens) and dropping fileDeltaStopWords. Rune-based
// length so a multi-byte CJK word ("根目录数闸门") is measured by character
// count, not bytes.
func itemKeywords(item string) []string {
	var out []string
	for _, w := range strings.FieldsFunc(strings.ToLower(item), func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r)
	}) {
		if len([]rune(w)) >= 4 && !fileDeltaStopWords[w] {
			out = append(out, w)
		}
	}
	return out
}
