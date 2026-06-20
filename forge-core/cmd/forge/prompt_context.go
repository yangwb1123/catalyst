// prompt_context.go — the cmd/forge side of the gate-result feedback loop. The
// orchestrator's Engine fires a GENERIC OnGateResult callback (it knows nothing
// of prompts); THIS file owns the prompt-shaped end of that wire: a gateLedger
// records each gate's latest objective verdict and renders it as a context block
// that buildPrompt injects into a LATER agent phase's prompt.
//
// Why it lives here, not in orchestrator: the orchestrator must stay free of any
// prompt/ledger concept (the same bright-line cost.go draws for claude-JSON cost).
// The engine REPORTS gate verdicts; cmd/forge — the only layer that builds the
// prompt — decides to remember them and feed them forward. This closes a real gap:
// without it the reviewer phase never sees that harness-gates already ran `test`
// and the complexity check, so under a print-mode agent (no Bash) it would blindly
// try to re-run them and burn its budget on permission denials. The injected text
// is the harness's OWN real verdicts (honesty-positive: a true signal, never a
// fabricated pass) — see context().
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/orchestrator"
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
	if l == nil || len(l.order) == 0 {
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

// phaseOutputSummaryCap bounds how many bytes of a fed-forward phase's output are
// remembered. The planner's output (a sprint split + acceptance criteria) can be long;
// injecting it whole into every later phase's prompt would bloat the prompt (and the
// token bill) without adding signal past the first screenful. record truncates beyond
// this cap and appends an ellipsis marker so the downstream agent knows it was clipped.
const phaseOutputSummaryCap = 800

// phaseOutputLedger accumulates the output of every phase whose asset declares
// feeds_forward, keyed by phase name, preserving first-seen order for a stable render.
// It is the in-memory bridge between the agent executor's Observe sink (which writes it,
// one phase at a time, for a feeds_forward phase only) and buildPrompt (which reads
// contextLines() into a LATER phase's prompt) — the EXACT structural mirror of
// gateLedger, but carrying planner-style planning output instead of gate verdicts.
//
// CONCURRENCY: deliberately lock-free, on the same premise as gateLedger — the
// orchestrator runs phases strictly sequentially, so record and contextLines never
// race. Should phases ever parallelize, this ledger would need a mutex too.
type phaseOutputLedger struct {
	summary map[string]string // phase name -> latest (truncated) output summary
	order   []string          // phase names in first-seen order (stable rendering)
}

// newPhaseOutputLedger returns an empty ledger ready to record phase outputs.
func newPhaseOutputLedger() *phaseOutputLedger {
	return &phaseOutputLedger{summary: map[string]string{}}
}

// record stores one phase's latest output, TRUNCATED to phaseOutputSummaryCap (with a
// trailing ellipsis when clipped) so a long planner output cannot bloat downstream
// prompts. It appends to the render order only the FIRST time a name is seen (a re-run
// phase — across loop-back or evolve iterations — updates its summary in place, keeping
// its original position), exactly like gateLedger.record. Safe on a nil receiver
// (no-op) so a caller that never constructed a ledger cannot panic.
func (l *phaseOutputLedger) record(phase, output string) {
	if l == nil {
		return
	}
	if _, seen := l.summary[phase]; !seen {
		l.order = append(l.order, phase)
	}
	l.summary[phase] = truncateSummary(output)
}

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

// context renders the recorded phase outputs as a prompt context block, or "" when the
// ledger is nil or empty (no feeds_forward phase has run yet). The text is HONEST about
// what this is: the prior PLANNING phase's own output (the planner's task split /
// acceptance criteria), offered to the implementer and reviewer as reference — NOT a
// gate verdict, NOT a fabricated fact. Each phase renders as a labeled block in
// first-seen order.
func (l *phaseOutputLedger) context() string {
	if l == nil || len(l.order) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString("前序规划阶段产出(planner 的任务拆分/验收标准,供实现与审查参考 —— 这是上游规划角色的客观产出,不是闸门结果):")
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
// CONCURRENCY: lock-free on the same premise as gateLedger/phaseOutputLedger — phases
// run strictly sequentially, so record and get never race.
type verdictLedger struct {
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
	l.verdict[phase] = verdict
}

// get reads back a phase's recorded verdict for Engine.AgentVerdict: (token, true) when
// one was recorded, ("", false) when none was — nil-safe, so an unwired ledger reports
// "no verdict" and the orchestrator proceeds (fail-open), never loops or panics.
func (l *verdictLedger) get(phase string) (string, bool) {
	if l == nil {
		return "", false
	}
	v, ok := l.verdict[phase]
	return v, ok
}

// reviewFindingsLedger carries the reviewer's findings text BACKWARD across a directed
// loop-back, to be injected into the IMPLEMENTER's next prompt for targeted repair. It
// is a deliberately ONE-DIRECTION edge: keyed by the loop-back TARGET phase (the
// implementer, read from the reviewer phase's on_fail.target — data-driven, zero
// hard-coded agent name), and buildPrompt injects it ONLY into that target phase. The
// reviewer, when it re-runs, has p.Name != target, so it NEVER receives these findings —
// preserving its fresh-context independence (the D3/AGENTS red line: a reviewer must not
// read its own prior self-report).
//
// CONCURRENCY: lock-free on the same sequential-phase premise as the sibling ledgers.
type reviewFindingsLedger struct {
	findings map[string]string // loop-back TARGET phase -> latest (truncated) findings
}

// newReviewFindingsLedger returns an empty ledger ready to record reviewer findings.
func newReviewFindingsLedger() *reviewFindingsLedger {
	return &reviewFindingsLedger{findings: map[string]string{}}
}

// record stores the reviewer's findings for the loop-back TARGET phase (its recipient),
// TRUNCATED to phaseOutputSummaryCap so a verbose review cannot bloat the implementer's
// prompt — reusing the same cap/marker as phaseOutputLedger. A re-review overwrites with
// the newest findings. Safe on a nil receiver (no-op).
func (l *reviewFindingsLedger) record(targetPhase, findings string) {
	if l == nil {
		return
	}
	l.findings[targetPhase] = truncateSummary(findings)
}

// contextLines renders the findings recorded for the phase named `name` as a single
// appendable prompt block, or nil when none were recorded for it (the common case — the
// gate is in buildPrompt, which only consults this for the implementer). The text is
// HONEST about provenance: the previous fresh-context Reviewer's per-finding
// REQUEST_CHANGES notes, offered for TARGETED repair — explicitly NOT a gate verdict.
func (l *reviewFindingsLedger) contextLines(name string) []string {
	if l == nil {
		return nil
	}
	f, ok := l.findings[name]
	if !ok || f == "" {
		return nil
	}
	return []string{"上一轮 fresh-context Reviewer 判 REQUEST_CHANGES 的逐条 findings(供定向修复参考;非闸门结果,是上游审查角色的判断):\n\n" + f}
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
//   - cost: ONLY for claude, the output is parsed for total_cost_usd and a billed figure
//     forwarded to costSink ALONG WITH the phase's routed model — echo/stubs never carry
//     that envelope, so they never bill. The model is resolved via phaseModel (closing
//     over wf+mode, the SAME orchestrator.PhaseTier handed to `claude --model`), because
//     the Observe seam is given only (phase NAME, output), never the Phase — the identical
//     reason feedsForward/onFailTarget are injected lookups rather than read off a Phase.
//
// Returns nil only when NO concern is live (not claude AND no other ledger wired),
// preserving the byte-for-byte no-hook default path for a plain stub run. unwrapClaudeResult
// (the log renderer) is applied by the caller, not here.
func observeFor(isClaude bool, costSink func(phase, model string, usd float64), phaseModel func(phase string) string, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool)) func(phase, output string) {
	if !isClaude && phaseOut == nil && verdicts == nil && findings == nil {
		return nil
	}
	return func(phase, output string) {
		if phaseOut != nil && feedsForward != nil && feedsForward(phase) {
			phaseOut.record(phase, unwrapClaudeResult(output))
		}
		if verdicts != nil {
			if v, ok := parseReviewerVerdict(output); ok {
				verdicts.record(phase, v)
				// On REQUEST_CHANGES, stash the findings for the loop-back target (the
				// implementer) — keyed off the phase's OWN on_fail.target, so the routing
				// is data-driven and the reviewer (a different phase) never receives them.
				if v == VerdictRequestChanges && findings != nil && onFailTarget != nil {
					if target, ok := onFailTarget(phase); ok {
						findings.record(target, unwrapClaudeResult(output))
					}
				}
			}
		}
		if isClaude && costSink != nil {
			if usd, ok := parseClaudeCostUsd(output); ok {
				costSink(phase, phaseModelOf(phaseModel, phase), usd)
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

// phaseModelResolver builds the (phase name -> routed model/tier) resolver the cost sink
// uses to attribute a billed cost to the model that incurred it. It mirrors feedsForwardOf /
// onFailTargetOf (closing over wf so the Observe seam, handed only a phase NAME, can look
// the phase up) and computes orchestrator.PhaseTier(p, mode) — the EXACT value
// agentExecutor's Build hands to `claude --model`, so the attributed model is the one the
// run actually routed to. An unknown phase name yields "" (omitempty drops it downstream).
func phaseModelResolver(wf asset.Workflow, mode string) func(name string) string {
	return func(name string) string {
		for _, p := range wf.Phases {
			if p.Name == name {
				return orchestrator.PhaseTier(p, mode)
			}
		}
		return ""
	}
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
func buildPrompt(repoRoot string, p asset.Phase, mode string, gates *gateLedger, phaseOut *phaseOutputLedger, findings *reviewFindingsLedger) string {
	tier := orchestrator.PhaseTier(p, mode)
	ctx := prompt.Gather(repoRoot, p.Name+" "+p.Agent)
	ctx = append(ctx, memoryContext(repoRoot)...)
	ctx = append(ctx, gates.contextLines()...)
	ctx = append(ctx, phaseOut.contextLines()...)
	// Gated on p.Name: findings.contextLines returns a block ONLY for the loop-back
	// target (the implementer). The reviewer, re-running with p.Name != target, gets
	// nil here — its fresh context is never polluted by its own earlier findings.
	ctx = append(ctx, findings.contextLines(p.Name)...)
	return prompt.Build(p.Agent, p.Name, mode, tier, readCard(repoRoot, p.Agent), ctx)
}

// memoryContext renders the cross-session store as one context block so the agent
// sees what prior iterations learned. Topic is unconstrained — a phase should see
// every gap/decision/lesson. Missing store = cold start (no block, no error); a
// malformed store is surfaced as a visible context line, not an aborted prompt.
func memoryContext(repoRoot string) []string {
	entries, err := memory.Load(memoryPath(repoRoot))
	if err != nil {
		return []string{"Project memory: UNREADABLE (" + err.Error() + ")"}
	}
	rel := memory.Query(entries, "", "")
	if len(rel) == 0 {
		return nil
	}
	var b strings.Builder
	b.WriteString("Project memory (gaps / decisions / lessons from prior iterations):")
	for _, e := range rel {
		fmt.Fprintf(&b, "\n- [%s] %s — %s (iter %d)", e.Kind, e.Topic, e.Detail, e.Iteration)
	}
	return []string{b.String()}
}

// readCard returns the agent's role-card text, or a short marker when absent so
// the prompt is still well-formed.
func readCard(repoRoot, agent string) string {
	b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agent+".md"))
	if err != nil {
		return fmt.Sprintf("(no role card found for %q)", agent)
	}
	return string(b)
}
