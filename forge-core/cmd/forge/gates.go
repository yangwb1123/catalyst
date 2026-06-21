// gates.go — the honest bridge from a workflow's REQUIRED gate names to their
// real per-gate verdicts. This is the fix for the FAKE PASS (FC-01): instead of
// collapsing lint/build/security onto one coarse aggregate result, each gate
// resolves to its OWN tri-state (PASS/FAIL/NA) using a single acceptance probe
// per run, and "gates green" means every required gate was actually CHECKED and
// PASSED — never merely "nothing failed".
package main

import (
	"fmt"
	"os"
	"path/filepath"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
)

// probeStatuses runs gate.ProbeAll once, returning the criterion->status map and
// the parallel criterion->category map (the lifecycle-aware N/A classification).
// On a probe error (e.g. node missing) it returns (nil, nil); downstream
// resolution treats an absent criterion as NA with an empty category, so a broken
// probe degrades to honest N/A — and, because an empty category is NOT exemptible
// (the matrix only waives a KNOWN inapplicable/no_tool), a broken probe can never
// be mistaken for a green convergence.
func probeStatuses(root string) (statuses, categories map[string]string) {
	statuses, categories, err := gate.ProbeAll(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: acceptance probe unavailable (%v); gates degrade to N/A\n", err)
		return nil, nil
	}
	return statuses, categories
}

// gatherSignals measures the live convergence inputs: the fraction of ROADMAP
// checklist items done, and whether EVERY required gate of the workflow's gate
// phases actually resolved to PASS. GatesGreen is true only when no required
// gate is FAIL and none is N/A — "green" means every required gate was CHECKED
// and PASSED, never merely "nothing failed".
//
// The same acceptance probe map (criterion -> PASS/FAIL/NA) is also handed to
// Signals.Criteria, so a workflow can converge on an INDIVIDUAL acceptance
// criterion (e.g. test_pass) and not only the coarse GatesGreen aggregate. This
// REUSES the once-per-run probe the gate phases already ran — acceptance is
// never spawned a second time for convergence; one probe feeds both the gate
// verdicts and the per-criterion convergence check, keeping them consistent and
// honest within a run. probe values are exactly gate.ProbeAll's PASS/FAIL/NA,
// which is the verdict vocabulary converge.evalCriterion expects; a nil probe
// (broken/absent) leaves Criteria nil and every per-criterion check degrades to
// unmet (absence of a verdict is never satisfaction).
func gatherSignals(root string, wf asset.Workflow, probe, categories map[string]string, lifecycle string, approved bool) converge.Signals {
	md, _ := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	green, proof := gatesGreen(root, requiredGates(wf), probe, categories, lifecycle)
	return converge.Signals{
		RoadmapCompletion: converge.RoadmapCompletion(string(md)),
		GatesGreen:        green,
		GateProof:         proof,
		Criteria:          probe,
		HumanApproved:     approved,
	}
}

// approvalPath is the on-disk human-approval marker for a stage: its mere
// EXISTENCE under <root>/.forge/<stage>.approved is one of the two approval
// signal sources (the other is the --approved flag). It lives in the git-ignored
// .forge runtime dir, so an approval is a deliberate local act, never committed.
func approvalPath(root, stage string) string {
	return filepath.Join(forgeDir(root), stage+".approved")
}

// humanApproved resolves the approval SIGNAL for a stage: true if the operator
// passed --approved OR a <root>/.forge/<stage>.approved marker exists. This is
// the v1 approval check — NOT a durable cross-process wait (durable_wait is v2,
// Temporal). It only reads the signal present right now; it does not block or
// persist a pending wait. fail-closed: with neither source the result is false,
// so an unapproved human_gate never auto-converges.
func humanApproved(root, stage string, flag bool) bool {
	if flag {
		return true
	}
	_, err := os.Stat(approvalPath(root, stage))
	return err == nil
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

// N/A category vocabulary (mirrors harness/acceptance-kernel.mjs). A criterion the
// LANGUAGE/project simply lacks is "inapplicable" (honest at every lifecycle); one
// whose TOOL is just missing/unconfigured is "no_tool" (a fixable gap, waived only
// while immature). Any other value (including "" from a pre-category probe or a
// broken probe) is NEITHER and is therefore never exempted — the strict default.
const (
	catInapplicable = "inapplicable"
	catNoTool       = "no_tool"
)

// exemptsNoTool reports whether a "no_tool" N/A may be waived at this lifecycle.
// ONLY the immature stages (idea/mvp/growth) qualify; production — and ANY unknown
// or empty lifecycle — does NOT (fail-safe toward strict: an unrecognised maturity
// is treated as production, so a missing tool blocks rather than silently passes).
// This is the lifecycle half of the two-dimensional matrix; it never applies to
// FAIL or to an "inapplicable" N/A (those have their own, lifecycle-independent
// rules in gatesGreen).
func exemptsNoTool(lifecycle string) bool {
	switch lifecycle {
	case "idea", "mvp", "growth":
		return true
	default: // production, unknown, "" -> strict
		return false
	}
}

// gatesGreen is the lifecycle-aware convergence judgment over the required gates,
// returning both the verdict and the per-gate proof (for honest reporting). It is
// the固化 of objective convergence for adapter-less languages WITHOUT weakening
// production, via a two-dimensional matrix over (status × category):
//
//	FAIL                      -> ALWAYS blocks (an exemption never applies to FAIL).
//	PASS                      -> proven (a real check ran and passed).
//	NA + inapplicable         -> exempt at ANY lifecycle (the language has no such
//	                             concept; nothing could make it applicable).
//	NA + no_tool              -> exempt IFF exemptsNoTool(lifecycle) — idea/mvp/
//	                             growth waive a fixable tooling gap; production /
//	                             unknown / "" require the tool (blocks).
//	NA + (any other category) -> blocks (strict default: a pre-category/broken probe
//	                             or an unclassified N/A is never waived).
//
// VACUOUS-GREEN GUARD (honesty): being "green" requires ≥1 non-NA gate that is a
// real PASS. If every required gate is N/A — even if all are exempted — nothing was
// proven, so the verdict is false. This upgrades the old "zero gates is not green"
// to "zero NON-NA gates is not green". Zero required gates is likewise not green.
func gatesGreen(root string, names []string, probe, categories map[string]string, lifecycle string) (bool, converge.GateProof) {
	var proof converge.GateProof
	if len(names) == 0 {
		return false, proof
	}
	provenCount := 0 // non-NA gates that PASSED (the vacuous-green guard's numerator)
	green := true
	for _, name := range names {
		res := resolveGate(root, name, probe)
		switch res.Status {
		case gate.StatusPass:
			provenCount++
			proof.Proven = append(proof.Proven, name)
		case gate.StatusNA:
			cat := gateCategory(name, categories)
			if !exemptNA(cat, lifecycle) {
				green = false // an un-waivable N/A (no_tool@production, or unknown) blocks
			} else {
				proof.Exemptions = append(proof.Exemptions, converge.GateExemption{
					Name: name, Category: cat, Reason: naReason(res, cat),
				})
			}
		default: // StatusFail — never exemptible
			green = false
		}
	}
	// Vacuous guard: a green verdict must rest on at least one proven (non-NA PASS)
	// gate; all-N/A (even fully exempted) proves nothing.
	if provenCount == 0 {
		green = false
	}
	return green, proof
}

// exemptNA applies the category half of the matrix: an "inapplicable" N/A is waived
// at any lifecycle, a "no_tool" N/A only when exemptsNoTool(lifecycle), and anything
// else (unknown/empty) is never waived.
func exemptNA(category, lifecycle string) bool {
	switch category {
	case catInapplicable:
		return true
	case catNoTool:
		return exemptsNoTool(lifecycle)
	default:
		return false
	}
}

// gateCategory resolves the N/A category for a gate by mapping its name onto the
// acceptance criterion that backs it (the same mapping resolveGate uses) and
// reading that criterion's category from the parallel ProbeAll map. The `test`
// gate combines test_pass+app_test_pass; an N/A there is driven by app_test_pass
// (the only one that can be N/A — no example apps), so its category is read from
// app_test_pass. An absent category yields "" (the strict, non-exemptible default).
func gateCategory(name string, categories map[string]string) string {
	criterion := name
	switch name {
	case "security":
		criterion = "security_findings"
	case "test":
		criterion = "app_test_pass"
	}
	return categories[criterion]
}

// naReason renders a short, honest reason for a waived N/A gate, preferring the
// acceptance probe's own detail and falling back to the category.
func naReason(res gate.Result, category string) string {
	if res.Output != "" {
		return res.Output
	}
	return category
}

// harnessRunner maps logical gate names onto their REAL per-gate verdict, using
// the once-per-run acceptance probe map. It is the fix for the FAKE PASS: each
// required gate resolves to its own PASS/FAIL/NA instead of every name sharing
// one coarse aggregate verdict.
func harnessRunner(repoRoot string, probe map[string]string) func(string) gate.Result {
	return func(name string) gate.Result {
		return resolveGate(repoRoot, name, probe)
	}
}

// resolveGate computes one gate's honest tri-state Result. Structural gates run
// their own tool live; the rest read the shared acceptance probe:
//
//	complexity -> gate.Gate  (structural caps, a real check)
//	arch       -> gate.Check (governance integrity, a real check)
//	test       -> PASS iff BOTH acceptance test_pass AND app_test_pass are PASS
//	lint       -> acceptance 'lint'            (N/A here: no linter)
//	build      -> acceptance 'build'           (N/A here: no build step)
//	security   -> acceptance 'security_findings' (N/A here: no scanner)
//
// An acceptance-backed gate whose criterion is missing from the probe map (or
// whose probe failed) resolves to N/A — honest "not checked", never a pass.
func resolveGate(repoRoot, name string, probe map[string]string) gate.Result {
	switch name {
	case "complexity":
		return gate.Gate(repoRoot)
	case "arch":
		return gate.Check(repoRoot)
	case "test":
		return combinedGate(name, probe, "test_pass", "app_test_pass")
	case "lint":
		return probedGate(name, probe, "lint")
	case "build":
		return probedGate(name, probe, "build")
	case "security":
		return probedGate(name, probe, "security_findings")
	default:
		return probedGate(name, probe, name)
	}
}

// probedGate reads one acceptance criterion's status from the probe map and
// renders it as a tri-state gate.Result. A criterion absent from the map means
// the probe did not report it (or did not run) -> N/A, never a silent pass.
func probedGate(name string, probe map[string]string, criterion string) gate.Result {
	status, ok := probe[criterion]
	if !ok {
		status = gate.StatusNA
	}
	return gate.Result{
		Name:   name,
		Status: status,
		OK:     status == gate.StatusPass,
		Output: probeDetail(criterion, status),
	}
}

// combinedGate ANDs several acceptance criteria into one gate. It is PASS only
// when every input criterion is PASS; if any input is N/A (and none FAIL) the
// gate is N/A; any FAIL makes it FAIL. This is how `test` requires BOTH the
// harness suites (test_pass) and the dogfood app suite (app_test_pass).
func combinedGate(name string, probe map[string]string, criteria ...string) gate.Result {
	status := gate.StatusPass
	for _, c := range criteria {
		switch probe[c] {
		case gate.StatusFail:
			status = gate.StatusFail
		case gate.StatusPass:
			// keep current (PASS unless already downgraded)
		default: // absent or NA
			if status != gate.StatusFail {
				status = gate.StatusNA
			}
		}
	}
	return gate.Result{
		Name:   name,
		Status: status,
		OK:     status == gate.StatusPass,
		Output: fmt.Sprintf("%v -> %s", criteria, status),
	}
}

// probeDetail renders a short, honest reason line for one resolved criterion.
func probeDetail(criterion, status string) string {
	switch status {
	case gate.StatusNA:
		return fmt.Sprintf("no executable check for %q in this repo", criterion)
	case gate.StatusFail:
		return fmt.Sprintf("acceptance criterion %q failed", criterion)
	default:
		return fmt.Sprintf("acceptance criterion %q passed", criterion)
	}
}

// loopProbe caches one acceptance probe per loop iteration. The repo changes
// between iterations (agents edit code), so the probe must be re-run each
// iteration — but the gate phases and the convergence check WITHIN an iteration
// must see the SAME probe. refresh() re-probes only when the cache is stale
// (first gate call of an iteration) and returns the status map; current() returns
// BOTH the status and category maps and marks the cache stale so the next
// iteration re-probes. This avoids double-spawning within an iteration while
// staying fresh across iterations, and keeps statuses+categories from the SAME run.
type loopProbe struct {
	root       string
	cached     map[string]string
	categories map[string]string
	primed     bool
}

// refresh runs gate.ProbeAll once per iteration (caching both the status and
// category maps) and returns the status map — the gate-resolution view, which only
// needs statuses. A probe error degrades both maps to nil (downstream treats absent
// criteria as N/A with an empty, non-exemptible category).
func (p *loopProbe) refresh() map[string]string {
	if !p.primed {
		statuses, categories, err := gate.ProbeAll(p.root)
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
	p.refresh()
	s, c := p.cached, p.categories
	p.primed = false
	return s, c
}
