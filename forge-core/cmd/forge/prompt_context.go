// prompt_context.go — the cmd/forge side of the gate-result feedback loop, and the
// seam where a finished phase's raw output funnels into the prompt-shaped state that
// LATER phases read back. The orchestrator's Engine fires a GENERIC OnGateResult
// callback (it knows nothing of prompts) and hands the executor a GENERIC Observe
// sink (it knows nothing of verdicts or feed-forward); THIS file owns the
// prompt-shaped end of both wires: a gateLedger records each gate's latest objective
// verdict and renders it as a context block that buildPrompt injects into a LATER
// agent phase's prompt, and observeFor composes the feed-forward / verdict / findings
// / cost concerns that react to a phase's raw output.
//
// Why it lives here, not in orchestrator: the orchestrator must stay free of any
// prompt/ledger concept (the same bright-line cost.go draws for claude-JSON cost).
// The engine REPORTS gate verdicts and raw phase output; cmd/forge — the only layer
// that builds the prompt — decides to remember them and feed them forward. This
// closes a real gap: without it the reviewer phase never sees that harness-gates
// already ran `test` and the complexity check, so under a print-mode agent (no Bash)
// it would blindly try to re-run them and burn its budget on permission denials. The
// injected text is the harness's OWN real verdicts (honesty-positive: a true signal,
// never a fabricated pass) — see context().
//
// The phaseOutputLedger, verdictLedger, reviewFindingsLedger, and the cross-session
// memory lane (memoryContext) are the SIBLING state this file's buildPrompt reads
// back — they live in prompt_memory.go, split out to keep both files under the
// volume budget (its package doc explains why). The workflow-DECLARED,
// never-FreshContext-gated artifact lanes (emits / uses_template /
// secondary_template) that buildPrompt also appends live in
// prompt_artifacts.go, split out for the same volume-budget reason.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"unicode"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/prompt"
)

// gateLedger accumulates the objective verdict of every gate the harness actually
// ran this run, keyed by gate name, preserving first-seen order for a stable
// render. It is the in-memory bridge between Engine.OnGateResult (which writes it,
// one gate at a time) and buildPrompt (which reads context() into a prompt).
//
// CONCURRENCY: this is deliberately lock-free. The orchestrator runs phases
// strictly sequentially (one phase, then the next; evolve drives RunFrom one
// iteration at a time), so record and context never race — there is no concurrent
// gate execution to guard against. Should the engine ever parallelize phases, this
// premise breaks and the ledger would need a mutex.
type gateLedger struct {
	mu     sync.Mutex        // guards status+order under the OPT-IN parallel orchestrator
	status map[string]string // gate name -> latest verdict ("ok" | "N/A" | "FAILED")
	order  []string          // gate names in first-seen order (stable rendering)
}

// newGateLedger returns an empty ledger ready to record gate verdicts.
func newGateLedger() *gateLedger {
	return &gateLedger{status: map[string]string{}}
}

// record stores one gate's latest verdict, appending to the render order only the
// FIRST time a name is seen (a re-run gate — e.g. across loop-back or evolve
// iterations — updates its status in place, keeping its original position). This is
// the method wired to Engine.OnGateResult. Safe on a nil receiver (no-op) so a
// caller that never constructed a ledger cannot panic.
func (l *gateLedger) record(name, status string) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if _, seen := l.status[name]; !seen {
		l.order = append(l.order, name)
	}
	l.status[name] = status
}

// context renders the recorded verdicts as a prompt context block, or "" when the
// ledger is nil or empty (no gate has run yet — the prior phases were all agent
// phases, or this is the first phase). The text states plainly that these are the
// harness's REAL, already-run results so a print-mode agent (no Bash) neither needs
// nor is able to re-run them — the whole point is to stop a reviewer from wasting
// its turn on permission-denied re-runs of checks the harness already objectively
// settled. Each gate renders as "- <name>: <verdict>" in first-seen order.
func (l *gateLedger) context() string {
	if l == nil {
		return ""
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	if len(l.order) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString("前序闸门结果(harness-gates 本轮已真实运行,客观事实 —— 你在 print 模式无 Bash,无需也无法自行重跑):")
	for _, name := range l.order {
		b.WriteString("\n- ")
		b.WriteString(name)
		b.WriteString(": ")
		b.WriteString(l.status[name])
	}
	b.WriteString("\n据此 + 代码 diff 静态判断;客观信号已由 harness 提供,勿试图自行执行命令验证。")
	return b.String()
}

// contextLines wraps context() as a slice ready to append onto a prompt's context
// lanes (nil when there is nothing to inject — symmetric with memoryContext), so
// buildPrompt stays a flat sequence of appends with no inline empty-check.
func (l *gateLedger) contextLines() []string {
	if c := l.context(); c != "" {
		return []string{c}
	}
	return nil
}

// feedsForwardOf returns a predicate that reports whether the phase named `name` in this
// workflow declares feeds_forward — the bridge from asset.Phase.FeedsForward to the
// executor's Observe sink, which is handed only a phase NAME (not the Phase). An unknown
// name (or a workflow where no phase sets the flag) yields false, so a workflow without
// any feeds_forward phase records nothing and the downstream prompt is unchanged.
func feedsForwardOf(wf asset.Workflow) func(name string) bool {
	return func(name string) bool {
		for _, p := range wf.Phases {
			if p.Name == name {
				return p.FeedsForward
			}
		}
		return false
	}
}

// onFailTargetOf returns a lookup from a phase NAME to its on_fail.loop_back target
// phase (the recipient of a directed loop-back — the implementer for build.yml's
// reviewer), or ("", false) when the phase carries no loop_back on_fail. It is the
// data-driven bridge (the structural mirror of feedsForwardOf) that lets the Observe
// sink route a reviewer's findings to the SAME phase the orchestrator will jump back to,
// with zero hard-coded agent name — read straight from the asset the author declared.
func onFailTargetOf(wf asset.Workflow) func(name string) (string, bool) {
	return func(name string) (string, bool) {
		for _, p := range wf.Phases {
			if p.Name == name && p.OnFail != nil && p.OnFail.Action == "loop_back" {
				return p.OnFail.TargetPhase, true
			}
		}
		return "", false
	}
}

// observeFor builds the executor's Observe sink — the seam where cmd/forge (the only
// vendor-aware layer) reacts to a finished phase's RAW output. It composes the
// independent concerns below, so the sink fires for echo as well as claude (feed-forward
// and the verdict state machine must work under the echo plumbing-check, not only a real
// claude run):
//   - feed-forward: when feedsForward(phase) is true, the phase's output (e.g. the
//     planner's task split) is recorded into phaseOut for injection into later prompts;
//   - verdict: the output's last line is parsed for the reviewer's VERDICT token and
//     recorded into verdicts (read back by Engine.AgentVerdict to drive loop-back); on a
//     REQUEST_CHANGES, the result text is also recorded into findings, keyed by this
//     phase's on_fail TARGET (the implementer), for targeted-repair injection;
//   - cost+latency: ONLY for claude, the output is parsed for total_cost_usd and a billed
//     figure forwarded to costSink ALONG WITH the phase's routed model AND the executor's
//     measured wall-clock latency — echo/stubs never carry that envelope, so they never bill
//     (and so never stamp a latency either; the wind-down's gate-on-real-cost then skips a
//     dry/echo run's scorecard entirely, so an un-billed phase's latency never reaches a
//     row). The model is resolved via phaseModel (closing over wf+mode, the SAME
//     orchestrator.PhaseTier handed to `claude --model`), because the Observe seam is given
//     only (phase NAME, output, latency), never the Phase — the identical reason
//     feedsForward/onFailTarget are injected lookups rather than read off a Phase. The
//     latency comes straight from the generic executor (a plain wall-clock duration it
//     measured), so this vendor-aware layer only RELAYS it to the cost stamp; it neither
//     measures nor interprets it.
//
// Returns nil only when NO concern is live (not claude AND no other ledger wired),
// preserving the byte-for-byte no-hook default path for a plain stub run. unwrapClaudeResult
// (the log renderer) is applied by the caller, not here. The latency argument is ignored on
// every non-cost concern (feed-forward, verdict) — only the billed claude path stamps it.
func observeFor(isClaude bool, costSink func(phase, model string, usd float64, latency time.Duration), phaseModel func(phase string) string, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool)) func(phase, output string, latency time.Duration) {
	if !isClaude && phaseOut == nil && verdicts == nil && findings == nil {
		return nil
	}
	return func(phase, output string, latency time.Duration) {
		sanitized := sanitizeAgentOutput(output)
		if phaseOut != nil && feedsForward != nil && feedsForward(phase) {
			phaseOut.record(phase, unwrapClaudeResult(sanitized))
		}
		if verdicts != nil {
			if v, ok := parseReviewerVerdict(sanitized); ok {
				verdicts.record(phase, v)
				// On REQUEST_CHANGES, stash the findings for the loop-back target (the
				// implementer) — keyed off the phase's OWN on_fail.target, so the routing
				// is data-driven and the reviewer (a different phase) never receives them.
				if v == VerdictRequestChanges && findings != nil && onFailTarget != nil {
					if target, ok := onFailTarget(phase); ok {
						findings.record(target, unwrapClaudeResult(sanitized))
					}
				}
			} else if v, ok := parseExecutiveVerdict(sanitized); ok {
				// The binary reviewer contract didn't match — try the CTO's 5-way
				// executive-review contract (review.yml P4) into the SAME ledger, so
				// Engine.AgentVerdict and reviewStatus (gates.go) read either kind back
				// through one uniform lookup. No findings side-effect: the executive
				// phase carries no on_fail.loop_back in review.yml (its rejection routes
				// via the workflow's own stop_condition.on_rejected, not a phase jump).
				verdicts.record(phase, v)
			} else if score, ok := parseConfidenceScore(sanitized); ok {
				// Neither the binary nor the 5-way token contract matched — try the
				// product-manager's numeric requirement-discovery contract (discover.yml
				// P1's CONFIDENCE: <N> last line). Stored as the plain numeric string
				// (e.g. "85") into the SAME ledger, so requirementConfidence (gates.go)
				// reads it back through the identical verdictLedger.get lookup the other
				// two tiers use — no findings side-effect (this phase carries no
				// on_fail.loop_back; discover.yml routes an unmet confidence via its own
				// stop_condition.on_unmet, not a phase jump).
				verdicts.record(phase, fmt.Sprintf("%.0f", score))
			}
		}
		if isClaude && costSink != nil {
			if usd, ok := parseClaudeCostUsd(output); ok {
				costSink(phase, phaseModelOf(phaseModel, phase), usd, latency)
			}
		}
	}
}

// phaseModelOf resolves the routed model/tier for a phase NAME via the injected
// phaseModel lookup, returning "" when none is wired (nil-safe). The empty string flows
// to costEmitter -> trace.Event.Model, where omitempty drops it — so a missing resolver
// degrades to an un-attributed cost event, never a panic or a fabricated model name.
func phaseModelOf(phaseModel func(phase string) string, phase string) string {
	if phaseModel == nil {
		return ""
	}
	return phaseModel(phase)
}

// sanitizeAgentOutput strips control characters and non-printable runes from agent
// output before it enters the feed-forward / verdict / findings pipeline
// (analysis §scan-new-angles direction 2: prompt injection attack surface).
// This prevents a compromised agent output from injecting control sequences or
// marker-syntax that could confuse downstream prompt construction. Characters
// kept: newline, tab, carriage-return, and all Unicode printable runes. Pure: no IO.
func sanitizeAgentOutput(output string) string {
	var b strings.Builder
	b.Grow(len(output))
	for _, r := range output {
		if r == '\n' || r == '	' || r == '\r' {
			b.WriteRune(r)
			continue
		}
		if unicode.IsPrint(r) {
			b.WriteRune(r)
		}
	}
	return strings.TrimSpace(b.String())
}

// contextMarker wraps a context block with a source marker that tells the agent
// what kind of information follows and that it is system-generated reference
// context, never a command to execute. This is a simple textual defense-in-depth
// against prompt injection: the marker is a plain prefix, not a boundary token,
// so it never interferes with prompt structure.
func contextMarker(source, content string) string {
	if content == "" {
		return ""
	}
	return "[context:" + source + "]\n" + content
}

// The cost path's (phase name -> routed model/tier) resolver lives in engine_build.go as
// phaseTierByName: it is the name-keyed face of the SHARED phaseTierResolver tierOf (which
// applies the near-budget down-tier on top of orchestrator.PhaseTier), so the model a cost is
// attributed to is byte-for-byte the one `claude --model` and the prompt used — one resolver,
// no drift. (It moved there, beside the resolver it wraps, when PR6 unified the three tier
// consumers; this file keeps only buildPrompt's prompt-tier consumption below.)

// gatherContext is the nil-safe dispatch for the invariant-context lanes: with no run cache
// it calls prompt.Gather (the unchanged, disk-every-time path — byte-for-byte the pre-cache
// behavior a nil-cache caller relies on); with a cache it calls prompt.GatherCached (same ctx
// slice, but the ADR/AGENTS lanes memoized for the run while the ROADMAP stays re-read). The
// returned slice is identical in both branches — the ONLY difference is whether the invariant
// inputs hit the filesystem this phase. Isolated as a one-liner so buildPrompt stays a flat
// append sequence and the back-compat branch is named in one obvious place.
func gatherContext(cache *prompt.ContextCache, repoRoot, query string) []string {
	if cache == nil {
		return prompt.Gather(repoRoot, query)
	}
	return prompt.GatherCached(cache, repoRoot, query)
}

// buildPrompt assembles the instruction for an agent phase. Beyond the role card,
// the Context Engine injects (1) hard constraints + ADRs RETRIEVED against this
// phase's query (Gather), (2) cross-session memory (memoryContext), (3) the
// objective verdicts of any gate the harness already ran this run (gates), (4) the
// output of any prior phase that declared feeds_forward — the planner's task split /
// acceptance criteria, fed to the implementer and reviewer (phaseOut), and (5) — ONLY
// when this phase is the loop-back recipient — the previous reviewer's REQUEST_CHANGES
// findings for targeted repair (findings, gated on p.Name so a re-running reviewer is
// NEVER fed its own prior notes, preserving fresh-context independence). A nil
// gates/phaseOut/findings add no blocks, so the prompt is byte-for-byte the old one.
//
// tierOf is the shared per-phase tier resolver (engine_build.go) — the SAME one Build
// hands to `claude --model` and the cost stamp reads — so the tier stated IN the prompt is
// exactly the model the phase actually runs, near-budget down-tier included. Passing the
// resolver (not a precomputed tier) keeps the single-source-of-truth: there is no second
// PhaseTier call here that could drift from the model the run spawns.
//
// cache is the run-scoped invariant-context memo (prompt.ContextCache, created per-run by
// buildRunEngine). It is OPTIONAL and nil-safe: a nil cache routes the INVARIANT-lane read
// through plain prompt.Gather (byte-for-byte the pre-cache path, so every existing caller /
// test that passes nil is unchanged); a non-nil cache routes it through prompt.GatherCached,
// which memoizes the ADR/AGENTS lanes across phases but ALWAYS re-reads the ROADMAP (the task
// lane is agent-writable and must never be served stale — the cache holds no field for it).
// Either way the ctx slice is identical; the cache only changes whether the invariant inputs
// are re-read from disk each phase. (HONESTY: this saves local readdir/readFile microseconds,
// NOT claude tokens — the prompt text is unchanged, so the full prompt is still billed every
// phase; the token win is v2's claude-API prompt-caching, for which this is the data-shape
// rehearsal — see internal/prompt/cache.go.)
func buildPrompt(repoRoot string, p asset.Phase, mode string, tierOf func(p asset.Phase) string, cache *prompt.ContextCache, gates *gateLedger, phaseOut *phaseOutputLedger, findings *reviewFindingsLedger) string {
	return buildPromptWithEmits(repoRoot, p, mode, tierOf, cache, gates, phaseOut, findings, nil)
}

// buildPromptWithEmits is like buildPrompt but also injects the content of
// files that prior phases declared via the `emits` field. The emitsFiles
// argument carries file path strings (from prior phases' Phase.Emits lists)
// relative to repoRoot — each existing file's content is read and injected
// as a [context:emit:...] block. A nil or empty emitsFiles is a no-op.
// This bridges the asset-runtime-gap §1.3: emitted artifacts (task-plan.md,
// proposal.md, etc.) are actually surfaced to downstream agent prompts.
//
// The lane assembly itself is split into appendFeedbackLanes (the FreshContext-gated
// memory/gate-results/phase-output/findings lanes) and appendArtifactContext (the
// always-on emits/uses_template lanes) so this function — and each helper — stays
// under the function-length budget; the append ORDER is unchanged from the original
// single-function version.
func buildPromptWithEmits(repoRoot string, p asset.Phase, mode string, tierOf func(p asset.Phase) string, cache *prompt.ContextCache, gates *gateLedger, phaseOut *phaseOutputLedger, findings *reviewFindingsLedger, emitsFiles []string) string {
	tier := tierOf(p)
	query := p.Name + " " + p.Agent
	ctx := gatherContext(cache, repoRoot, query)
	// Inject phase description as the first context lane, before the role card
	// (asset-runtime-gap §1.5). These 2-5 line descriptions carry workflow-level
	// context about what THIS phase should do in THIS workflow, which is more
	// specific than the generic agent card.
	if p.Description != "" {
		ctx = append(ctx, "## Current phase description\n"+p.Description)
	}
	ctx = appendFeedbackLanes(ctx, repoRoot, query, p, gates, phaseOut, findings)
	ctx = appendArtifactContext(ctx, repoRoot, emitsFiles, p.UsesTemplate, p.SecondaryTemplate)
	return prompt.Build(p.Agent, p.Name, mode, tier, readCard(repoRoot, p.Agent, cache), ctx)
}

// appendFeedbackLanes appends the memory / gate-results / phase-output / findings
// context lanes onto ctx, each wrapped with its [context:source] marker — a defense-in-
// depth against prompt injection, so an agent can always distinguish system-provided
// reference context from commands or instructions (scan-new-angles §方向2).
//
// FreshContext enforcement: when p.FreshContext is true, this is a NO-OP — gate results,
// phase outputs, memory, and findings are all skipped, giving this phase a clean slate as
// declared by the workflow author (asset-runtime-gap §1.2). This implements the
// engineering rule that a Reviewer must be a fresh-context independent agent that does
// not see the implementer's prior output or gate results, preventing anchoring bias.
func appendFeedbackLanes(ctx []string, repoRoot, query string, p asset.Phase, gates *gateLedger, phaseOut *phaseOutputLedger, findings *reviewFindingsLedger) []string {
	if p.FreshContext {
		return ctx
	}
	if mc := memoryContext(repoRoot, query); len(mc) > 0 {
		ctx = append(ctx, contextMarker("memory", mc[0]))
	}
	if gc := gates.contextLines(); len(gc) > 0 {
		ctx = append(ctx, contextMarker("gate-results", gc[0]))
	}
	if pc := phaseOut.contextLines(); len(pc) > 0 {
		ctx = append(ctx, contextMarker("phase-output", pc[0]))
	}
	// Gated on p.Name: findings.contextLines returns a block ONLY for the loop-back
	// target (the implementer). The reviewer, re-running with p.Name != target, gets
	// nil here — its fresh context is never polluted by its own earlier findings.
	if fc := findings.contextLines(p.Name); len(fc) > 0 {
		ctx = append(ctx, contextMarker("findings", fc[0]))
	}
	return ctx
}

// appendArtifactContext (emits / uses_template / secondary_template — never gated
// on FreshContext) lives in prompt_artifacts.go, split out to keep this file under
// the volume budget.

// readCard returns the agent's role-card text, or a short marker when absent so
// the prompt is still well-formed. Uses the ContextCache when non-nil (§3.4).
func readCard(repoRoot, agent string, cache *prompt.ContextCache) string {
	if cache != nil {
		return cache.CardText(agent)
	}
	b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agent+".md"))
	if err != nil {
		return fmt.Sprintf("(no role card found for %q)", agent)
	}
	return string(b)
}

// requiresToolsGuard implements discover.yml's requires_tools degrade-and-flag
// contract (market-research: requires_tools: [web_search, web_fetch] — "无检索工具
// 则降级 advisory 并打标"). forge-core has NO live tool probe, so this decides from
// STATIC executor config alone, caller-supplied: isCommandExec (false under
// DryRunExecutor — narrate-only, no live tool call exists at all), isClaude (false
// under a non-claude command executor, e.g. the echo plumbing check), and
// allowedTools (this run's claude --allowedTools whitelist string). A required tool
// (snake_case, e.g. "web_search") is CONFIRMED only when its alias — the name
// collapsed to one word, "websearch" — appears in allowedTools, matching how a
// caller would actually whitelist claude's WebSearch/WebFetch tools. Absence of
// proof is never treated as proof of absence: every branch that cannot
// affirmatively confirm returns a reason, never a silent guess either way.
//
// When every required tool is confirmed, text returns byte-for-byte UNCHANGED
// (the common, silent path — every phase without requires_tools, and a
// fully-confirmed one, are both no-ops). Otherwise it logs a visible ⚠ degrade
// line via logln (mirrors loop.go's honesty-warning convention) and appends a
// [context:requires_tools] block (reusing contextMarker's prompt-injection-safe
// prefix) telling the agent to mark unresearched claims advisory/unverified/
// uncited — never a silent pass-through of unfounded claims, never a hard-fail.
func requiresToolsGuard(p asset.Phase, isCommandExec, isClaude bool, allowedTools string, logln func(string), text string) string {
	if len(p.RequiresTools) == 0 {
		return text
	}
	reason := ""
	switch {
	case !isCommandExec:
		reason = "dry-run executor narrates only — no live tool calls exist"
	case !isClaude:
		reason = "non-claude command executor — no live tool access"
	case allowedTools == "":
		reason = "no --allowedTools whitelist configured — cannot verify tool grant"
	default:
		lower := strings.ToLower(allowedTools)
		var missing []string
		for _, t := range p.RequiresTools {
			alias := strings.ReplaceAll(strings.ToLower(t), "_", "")
			if !strings.Contains(lower, alias) {
				missing = append(missing, t)
			}
		}
		if len(missing) == 0 {
			return text // confirmed: every required tool named in --allowedTools
		}
		reason = "not confirmed in --allowedTools: " + strings.Join(missing, ", ")
	}
	if logln != nil {
		logln(fmt.Sprintf("forge: ⚠ phase %s requires_tools=%v not confirmed (%s) — degrading to advisory", p.Name, p.RequiresTools, reason))
	}
	note := contextMarker("requires_tools", fmt.Sprintf("Declared requires_tools=%v could not be confirmed available this run (%s). If you cannot actually search/fetch, mark affected claims advisory/unverified/uncited rather than presenting them as confirmed fact.", p.RequiresTools, reason))
	return text + "\n\n" + note
}
