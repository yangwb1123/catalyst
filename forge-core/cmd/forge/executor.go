// executor.go — agent-executor factory, claude-argv construction, and read-only
// path enforcement for `forge run` / `forge evolve`. These functions build the
// orchestrator.AgentExecutor (agentExecutor) and the per-phase claude command-line
// arguments with tier, permission mode, and read-only path scoping (claudeArgv,
// mergeToolList, readonlyAgentWriteScope, readonlyToolScope, narrateReadonly).
// Split out of engine_build.go to keep both files under the per-file volume budget.
package main

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/prompt"
	"forgeos/forge-core/internal/routing"
)

// agentExecutor builds the orchestrator.AgentExecutor closure for a run. It
// selects the COMMAND executor (spawns a real agent) when --executor=command,
// else the DRY-RUN executor (narrates only). The command executor wires the
// agent's argv, prompt, cost-observation sink, and the six ledgers (gates/
// phase-output/feeds-forward/verdicts/findings/on-fail-target) into a single
// callable the orchestrator invokes per phase.
func agentExecutor(o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), tierOf func(p asset.Phase) string, phaseModel func(phase string) string, ctxCache *prompt.ContextCache, gates *gateLedger, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool), priorEmits func(phase string) []string, outputHooks ...executorHooks) orchestrator.AgentExecutor {
	if o.executor != "command" {
		return orchestrator.DryRunExecutor{Log: logln}
	}
	isClaude := isClaudeExecutable(o.agentCmd)
	config := commandExecutorConfig{
		opts: o, logln: logln, costSink: costSink, tierOf: tierOf,
		phaseModel: phaseModel, ctxCache: ctxCache, gates: gates,
		phaseOut: phaseOut, feedsForward: feedsForward, verdicts: verdicts,
		findings: findings, onFailTarget: onFailTarget, priorEmits: priorEmits,
		hooks: firstExecutorHooks(outputHooks), isClaude: isClaude,
		releaseAgent: bindReleaseAgent(
			o.root, o.agentCmd, o.releaseAgentPath, o.releaseAgentSHA256,
		),
		releasePrompts: newReleasePromptCache(), lifecycle: resolveLifecycle(o),
	}
	return config.executor()
}

func isClaudeExecutable(command string) bool {
	return filepath.Base(strings.TrimSpace(command)) == "claude"
}

type commandExecutorConfig struct {
	opts           runOpts
	logln          func(string)
	costSink       func(phase, model string, usd float64, latency time.Duration)
	tierOf         func(asset.Phase) string
	phaseModel     func(string) string
	ctxCache       *prompt.ContextCache
	gates          *gateLedger
	phaseOut       *phaseOutputLedger
	feedsForward   func(string) bool
	verdicts       *verdictLedger
	findings       *reviewFindingsLedger
	onFailTarget   func(string) (string, bool)
	priorEmits     func(string) []string
	hooks          executorHooks
	isClaude       bool
	releaseAgent   releaseAgentBinding
	releasePrompts *releasePromptCache
	lifecycle      string
}

func (c commandExecutorConfig) executor() orchestrator.AgentExecutor {
	restrictedEnv := restrictedAgentEnvironment(c.opts)
	ex := orchestrator.CommandExecutor{
		ValidateConfig:    c.validate,
		Build:             c.build,
		Dir:               c.opts.root,
		Timeout:           c.opts.timeout,
		MaxDepth:          c.opts.maxAgentDepth,
		MaxOutputBytes:    c.opts.maxOutputBytes,
		EnvAllow:          agentEnvAllow(c.isClaude, restrictedEnv, c.opts.agentEnv),
		RestrictedEnv:     restrictedEnv,
		PromptViaStdin:    c.isClaude,
		Log:               c.logln,
		ValidateOutput:    c.hooks.ValidateOutput,
		ValidateRawOutput: c.hooks.ValidateRawOutput,
	}
	phaseModel := preferPhaseModel(c.hooks.ModelFor, c.phaseModel)
	ex.Observe = observeFor(c.isClaude, c.costSink, phaseModel, c.phaseOut, c.feedsForward, c.verdicts, c.findings, c.onFailTarget, c.hooks.VerdictContractFor, c.hooks.ScanContractFor)
	if c.isClaude {
		ex.RenderLog = unwrapClaudeResult
		ex.ClassifyOverload = classifyClaudeOverload
	}
	return ex
}

func restrictedAgentEnvironment(o runOpts) bool {
	return o.evolveProposalOnly || o.workflowStage == "deploy" || o.workflowStage == "rollback"
}

func (c commandExecutorConfig) validate(p asset.Phase, _ string) error {
	if err := validateProposalExecutor(p, c.opts, c.isClaude, c.releaseAgent); err != nil {
		return err
	}
	if p.Readonly && p.Agent != "release-engineer" {
		if _, err := readonlyEmitPermissionPatterns(c.opts.root, p); err != nil {
			return err
		}
	}
	if err := validateReleaseExecutor(p, c.opts, c.isClaude, c.releaseAgent); err != nil {
		return err
	}
	if p.Agent == "release-engineer" {
		rework := len(c.findings.contextLines(p.Name)) > 0
		return c.releasePrompts.prepare(c.opts.root, p, rework)
	}
	return nil
}

func (c commandExecutorConfig) build(p asset.Phase, runMode string) []string {
	c.verdicts.clear(p.Name)
	if p.WritesADR != nil {
		authorized, reason := effectiveWritesADR(c.opts.root, runMode, c.lifecycle, p.WritesADR)
		if reason != "" && c.logln != nil {
			c.logln(fmt.Sprintf("phase %s: writes_adr disabled: %s", p.Name, reason))
		}
		p.WritesADR = authorized
	}
	narrateReadonly(c.logln, p)
	model := c.tierOf(p)
	argv := claudeArgv(trustedAgentOpts(c.opts, p, c.releaseAgent), c.isClaude, model, p)
	if p.Agent == "release-engineer" || c.opts.evolveProposalOnly {
		argv = c.releaseAgent.wrapArgv(argv)
	}
	frozenTier := func(asset.Phase) string { return model }
	var text, frozenSourceRevision string
	var frozenReleaseInputs map[string]string
	if p.Agent == "release-engineer" {
		var ok bool
		text, frozenSourceRevision, frozenReleaseInputs, ok = c.releasePrompts.build(p, runMode, model)
		if !ok {
			return nil
		}
	} else {
		text = buildPromptWithEmits(c.opts.root, p, runMode, frozenTier, c.ctxCache, c.gates, c.phaseOut, c.findings, emitsFilesFor(c.priorEmits, p.Name))
	}
	text = appendEvolveScanPrompt(text, p, c.hooks.ScanDepth)
	text = requiresToolsGuard(p, true, c.isClaude, c.opts.agentAllowedTools, c.logln, text)
	if c.hooks.OnBuild != nil {
		c.hooks.OnBuild(p, model, text, frozenSourceRevision, frozenReleaseInputs)
	}
	return append(argv, "-p", text)
}

type executorHooks struct {
	ValidateOutput     func(phase, output string) error
	ValidateRawOutput  func(phase, output string) error
	OnBuild            func(phase asset.Phase, model, promptText, frozenSourceRevision string, frozenReleaseInputs map[string]string)
	ModelFor           func(phase string) string
	VerdictContractFor func(phase string) string
	ScanContractFor    func(phase string) string
	ScanDepth          string
}

func firstExecutorHooks(hooks []executorHooks) executorHooks {
	if len(hooks) == 0 {
		return executorHooks{}
	}
	return hooks[0]
}

func preferPhaseModel(primary, fallback func(string) string) func(string) string {
	return func(name string) string {
		if model := phaseModelOf(primary, name); model != "" {
			return model
		}
		return phaseModelOf(fallback, name)
	}
}

var claudeCredentialEnv = []string{
	"ANTHROPIC_API_KEY",
	"ANTHROPIC_AUTH_TOKEN",
	"ANTHROPIC_BASE_URL",
	"CLAUDE_CODE_OAUTH_TOKEN",
	"CLAUDE_CONFIG_DIR",
}

// agentEnvAllow grants only the credentials required by the selected agent.
// Extra variables require an operator's exact-name --agent-env authorization.
func agentEnvAllow(isClaude, restricted bool, extra string) []string {
	var allowed []string
	if isClaude {
		for _, name := range claudeCredentialEnv {
			if !restricted || name != "CLAUDE_CONFIG_DIR" {
				allowed = append(allowed, name)
			}
		}
	}
	if restricted {
		return allowed
	}
	for _, name := range strings.Split(extra, ",") {
		if name = strings.TrimSpace(name); name != "" {
			allowed = append(allowed, name)
		}
	}
	return allowed
}

// claudeArgv builds the leading argv (everything before `-p <prompt>`) for an agent
// command: [agentCmd] for a non-claude command (echo/stubs, back-compat), else the
// print-mode flags: --permission-mode, --disallowedTools/--allowedTools for read-only
// enforcement, --model <tier>, --max-budget-usd, and --output-format json.
func claudeArgv(o runOpts, isClaude bool, tier string, p asset.Phase) []string {
	argv := []string{o.agentCmd}
	if !isClaude {
		return argv
	}
	argv = claudeCommandPrefix(o, p)
	permissionMode := o.agentPermission
	if p.Readonly {
		// acceptEdits would auto-approve every repository edit and defeat the
		// path allowlist. dontAsk denies every call not explicitly pre-approved.
		permissionMode = "dontAsk"
	}
	if permissionMode != "" {
		argv = append(argv, "--permission-mode", permissionMode)
	}
	deny, allowExtra := readonlyToolScopeForRoot(o.root, p)
	if o.evolveProposalOnly && p.Readonly {
		deny = mergeToolList(deny, "Bash NotebookEdit WebFetch WebSearch")
	}
	if deny != "" {
		argv = append(argv, "--disallowedTools", deny)
	}
	baseAllowed := o.agentAllowedTools
	if o.evolveProposalOnly && p.Readonly {
		// Proposal-only is a capability boundary, not a prompt suggestion.
		// Operator/default Bash grants could write outside exact emits, so remove
		// them and retain only the path-scoped Edit grants assembled below.
		baseAllowed = ""
	}
	if p.Agent == "release-engineer" {
		// The ordinary default contains Bash validators. Release work is local,
		// declarative, and docs-only, so it receives only its path-scoped
		// Edit grant; remote/shell tools remain explicitly denied.
		baseAllowed = ""
	}
	if allowed := mergeToolList(baseAllowed, allowExtra); allowed != "" {
		argv = append(argv, "--allowedTools", allowed)
	}
	argv = append(argv, "--model", tier)
	if o.agentMaxBudgetUSD != "" {
		argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
	}
	return append(argv, "--output-format", "json")
}

func claudeCommandPrefix(o runOpts, p asset.Phase) []string {
	argv := []string{o.agentCmd}
	tools := ""
	switch {
	case p.Agent == "release-engineer":
		tools = "Edit,Write"
	case o.evolveProposalOnly:
		tools = "Read,Glob,Grep,Edit,Write"
	default:
		return argv
	}
	// Capability-restricted phases suppress ambient settings, hooks, skills,
	// plugins and MCP. The built-in set contains no shell or network tool.
	return append(argv,
		"--bare", "--safe-mode", "--strict-mcp-config",
		"--disable-slash-commands", "--no-chrome", "--no-session-persistence",
		"--tools", tools,
	)
}

// mergeToolList joins the operator whitelist (base) and a readonly phase's
// exact Edit pre-approval patterns (extra) into ONE space-separated --allowedTools
// value. Either half may be empty; "" only when both are.
func mergeToolList(base, extra string) string {
	switch {
	case base == "":
		return extra
	case extra == "":
		return base
	default:
		return base + " " + extra
	}
}

// readonlyAgentWriteScope maps an agent to the claude permission-specifier
// PATTERN(s) (gitignore-style, project-root-relative) its OWN card documents
// as its sole write target. An agent ABSENT here has NO documented target and
// stays FULLY denied when readonly.
var readonlyAgentWriteScope = map[string][]string{
	"product-manager":      {"/docs/discovery/**"},
	"researcher":           {"/docs/discovery/**"},
	"architect":            {"/docs/design/**"},
	"cto":                  {"/docs/design/**"},
	"planner":              {"/.agent/CURRENT_SPRINT.md", "/.agent/ROADMAP.md"},
	"security-engineer":    {"/docs/review/**"},
	"distributed-engineer": {"/docs/review/**"},
	"performance-engineer": {"/docs/review/**"},
	"qa":                   {"/docs/review/eval-scorecard.md"},
}

// readonlyToolScope returns the --disallowedTools value and any --allowedTools
// re-open patterns for a phase. Non-readonly -> ("", ""). A readonly phase
// runs under dontAsk and pre-approves Edit only for its agent's documented
// target plus a validated ADR directory. Edit(path) is Claude's canonical rule
// for every built-in writing tool; Write(path) is intentionally never emitted.
func readonlyToolScope(p asset.Phase) (deny, allow string) {
	return readonlyToolScopeForRoot(".", p)
}

// readonlyToolScopeForRoot is the production variant: it re-opens an ADR
// directory only after condition syntax and repo containment have been checked.
// agentExecutor has already removed WritesADR when mode/lifecycle disallow it.
func readonlyToolScopeForRoot(repoRoot string, p asset.Phase) (deny, allow string) {
	if !p.Readonly {
		return "", ""
	}
	if p.Agent == "release-engineer" {
		deny = "Bash WebFetch WebSearch"
		patterns, err := releaseEmitPermissionPatterns(repoRoot, p)
		if err != nil {
			return deny, ""
		}
		specs := make([]string, 0, len(patterns))
		for _, pattern := range patterns {
			specs = append(specs, "Edit("+pattern+")")
		}
		return deny, strings.Join(specs, " ")
	}
	patterns, err := readonlyEmitPermissionPatterns(repoRoot, p)
	if err != nil {
		return deny, ""
	}
	if p.WritesADR != nil && p.WritesADR.Target != "" {
		if _, err := parseWritesADRCondition(p.WritesADR.Condition); err == nil {
			if _, relTarget, err := containedADRTarget(repoRoot, p.WritesADR.Target); err == nil {
				patterns = append(patterns, "/"+strings.Trim(relTarget, "/")+"/**")
			}
		}
	}
	if len(patterns) == 0 {
		return deny, ""
	}
	specs := make([]string, 0, len(patterns))
	for _, pat := range patterns {
		specs = append(specs, "Edit("+pat+")")
	}
	return deny, strings.Join(specs, " ")
}

// readonlyEmitPermissionPatterns narrows a role's documented write ceiling to
// the phase's exact, validated emits. A readonly agent with no emits receives no
// ordinary write grant. Containment, symlink prefixes, permission-pattern
// characters, and the role ceiling are all checked before command spawn.
func readonlyEmitPermissionPatterns(repoRoot string, p asset.Phase) ([]string, error) {
	if len(p.Emits) == 0 {
		return nil, nil
	}
	ceiling := readonlyAgentWriteScope[p.Agent]
	if len(ceiling) == 0 {
		return nil, fmt.Errorf("readonly phase %q agent %q has emits but no documented write scope", p.Name, p.Agent)
	}
	patterns := make([]string, 0, len(p.Emits))
	seen := make(map[string]bool, len(p.Emits))
	for _, emit := range p.Emits {
		absolute, relative, err := containedRepoPath(repoRoot, emit)
		if err != nil {
			return nil, fmt.Errorf("readonly phase %q emit %q: %w", p.Name, emit, err)
		}
		if err := validateReadonlyEmitIdentity(repoRoot, absolute); err != nil {
			return nil, fmt.Errorf("readonly phase %q emit %q: %w", p.Name, emit, err)
		}
		relative = filepath.ToSlash(relative)
		if !safeADRRelativePath(relative) {
			return nil, fmt.Errorf("readonly phase %q emit %q contains unsafe permission-pattern characters", p.Name, emit)
		}
		exact := "/" + strings.TrimPrefix(relative, "/")
		if !withinReadonlyCeiling(exact, ceiling) {
			return nil, fmt.Errorf("readonly phase %q emit %q is outside agent %q write scope", p.Name, emit, p.Agent)
		}
		if !seen[exact] {
			patterns = append(patterns, exact)
			seen[exact] = true
		}
	}
	return patterns, nil
}

func withinReadonlyCeiling(exact string, ceiling []string) bool {
	for _, allowed := range ceiling {
		if strings.HasSuffix(allowed, "/**") {
			if strings.HasPrefix(exact, strings.TrimSuffix(allowed, "**")) {
				return true
			}
			continue
		}
		if exact == allowed {
			return true
		}
	}
	return false
}

// narrateReadonly logs a decision-narration line when a readonly phase is
// spawned under --executor=command. Purely observational — the actual
// restriction is claudeArgv's readonlyToolScope.
func narrateReadonly(logln func(string), p asset.Phase) {
	if !p.Readonly || logln == nil {
		return
	}
	emits := "none declared"
	if len(p.Emits) > 0 {
		emits = strings.Join(p.Emits, ", ")
	}
	logln(fmt.Sprintf("phase %s: readonly=true (analysis-only — must not modify existing source/product code; MAY still write its declared emits: %s) — ENFORCED: dontAsk denies unapproved tools; Edit is pre-approved only for this phase's validated write scope (see readonlyToolScope)", p.Name, emits))
}

// taskAwareTierResolver lets a validated planner TASK_LIST raise the model for
// the first dependency-ready implementation task. It never lowers the resolver's
// safety, risk, budget, or history result.
func taskAwareTierResolver(base func(asset.Phase) string, plans *phaseOutputLedger, logln func(string)) func(asset.Phase) string {
	return func(p asset.Phase) string {
		tier := base(p)
		if p.Agent != "implementer" {
			return tier
		}
		hint := plans.recommendedTaskModel()
		if hint == "" {
			return tier
		}
		chosen := routing.Higher(tier, hint)
		if logln != nil {
			relation := "kept"
			if chosen != tier {
				relation = "raised"
			}
			logln(fmt.Sprintf("phase %s: planner TASK_LIST model=%s — %s routed tier %s→%s",
				p.Name, hint, relation, tier, chosen))
		}
		return chosen
	}
}

// loopbackVerdict preserves the ledger's exact executive verdict for reporting,
// while adapting negative CTO outcomes to the orchestrator's vendor-neutral
// REQUEST_CHANGES transition token.
func loopbackVerdict(verdicts *verdictLedger) func(string) (string, bool) {
	return func(phase string) (string, bool) {
		v, ok := verdicts.get(phase)
		if !ok {
			return "", false
		}
		switch v {
		case VerdictRedesign, VerdictDelay, VerdictReject:
			return VerdictRequestChanges, true
		default:
			return v, true
		}
	}
}

// recordLoopbackFindings sends repair context to the phase declared as the
// review phase's loop-back target. Binary reviewer changes and negative CTO
// verdicts share the same data-driven path.
func recordLoopbackFindings(phase, verdict, output string, findings *reviewFindingsLedger, targetOf func(string) (string, bool)) {
	if findings == nil || targetOf == nil {
		return
	}
	switch verdict {
	case VerdictRequestChanges, VerdictRedesign, VerdictDelay, VerdictReject:
	default:
		return
	}
	if target, ok := targetOf(phase); ok {
		findings.record(target, unwrapClaudeResult(output))
	}
}
