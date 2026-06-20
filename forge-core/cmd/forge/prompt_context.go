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

// observeFor builds the executor's Observe sink — the seam where cmd/forge (the only
// vendor-aware layer) reacts to a finished phase's RAW output. It composes two
// independent concerns, so the sink fires for echo as well as claude (feed-forward must
// work under the echo plumbing-check, not just a real claude run):
//   - feed-forward: when feedsForward(phase) is true, the phase's output (e.g. the
//     planner's task split) is recorded into phaseOut for injection into later prompts;
//   - cost: ONLY for claude, the output is parsed for total_cost_usd and a billed figure
//     forwarded to costSink — echo/stubs never carry that envelope, so they never bill.
//
// Returns nil only when NEITHER concern is live (not claude AND no feeds_forward ledger),
// preserving the byte-for-byte no-hook default path for a plain stub run with no
// feed-forward. unwrapClaudeResult (the log renderer) is applied by the caller, not here.
func observeFor(isClaude bool, costSink func(phase string, usd float64), phaseOut *phaseOutputLedger, feedsForward func(phase string) bool) func(phase, output string) {
	if !isClaude && phaseOut == nil {
		return nil
	}
	return func(phase, output string) {
		if phaseOut != nil && feedsForward != nil && feedsForward(phase) {
			phaseOut.record(phase, unwrapClaudeResult(output))
		}
		if isClaude && costSink != nil {
			if usd, ok := parseClaudeCostUsd(output); ok {
				costSink(phase, usd)
			}
		}
	}
}

// buildPrompt assembles the instruction for an agent phase. Beyond the role card,
// the Context Engine injects (1) hard constraints + ADRs RETRIEVED against this
// phase's query (Gather), (2) cross-session memory (memoryContext), (3) the
// objective verdicts of any gate the harness already ran this run (gates), and (4)
// the output of any prior phase that declared feeds_forward — the planner's task
// split / acceptance criteria, fed to the implementer and reviewer (phaseOut). A nil
// gates AND a nil/empty phaseOut add no blocks, so the prompt is byte-for-byte the old one.
func buildPrompt(repoRoot string, p asset.Phase, mode string, gates *gateLedger, phaseOut *phaseOutputLedger) string {
	tier := orchestrator.PhaseTier(p, mode)
	ctx := prompt.Gather(repoRoot, p.Name+" "+p.Agent)
	ctx = append(ctx, memoryContext(repoRoot)...)
	ctx = append(ctx, gates.contextLines()...)
	ctx = append(ctx, phaseOut.contextLines()...)
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
