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

// probeStatuses runs gate.ProbeAll once, returning the criterion->status map.
// On a probe error (e.g. node missing) it returns nil; downstream resolution
// treats an absent criterion as NA, so a broken probe degrades to honest N/A
// rather than a fake pass.
func probeStatuses(root string) map[string]string {
	statuses, err := gate.ProbeAll(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: acceptance probe unavailable (%v); gates degrade to N/A\n", err)
		return nil
	}
	return statuses
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
func gatherSignals(root string, wf asset.Workflow, probe map[string]string) converge.Signals {
	md, _ := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	return converge.Signals{
		RoadmapCompletion: converge.RoadmapCompletion(string(md)),
		GatesGreen:        allRequiredGatesPass(root, requiredGates(wf), probe),
		Criteria:          probe,
	}
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

// allRequiredGatesPass reports whether every required gate resolves to PASS via
// the shared resolver. Zero required gates is NOT green (nothing was proven).
func allRequiredGatesPass(root string, names []string, probe map[string]string) bool {
	if len(names) == 0 {
		return false
	}
	for _, name := range names {
		if resolveGate(root, name, probe).Status != gate.StatusPass {
			return false
		}
	}
	return true
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
// (first gate call of an iteration); current() returns the cached map and marks
// it stale so the next iteration re-probes. This avoids double-spawning within
// an iteration while staying fresh across iterations.
type loopProbe struct {
	root   string
	cached map[string]string
	primed bool
}

// refresh returns the probe map, running gate.ProbeAll once per iteration. A
// probe error degrades to nil (downstream treats absent criteria as N/A).
func (p *loopProbe) refresh() map[string]string {
	if !p.primed {
		statuses, err := gate.ProbeAll(p.root)
		if err != nil {
			fmt.Fprintf(os.Stderr, "forge evolve: acceptance probe unavailable (%v); gates degrade to N/A\n", err)
			statuses = nil
		}
		p.cached = statuses
		p.primed = true
	}
	return p.cached
}

// current returns the iteration's probe (priming it if gates somehow did not),
// then marks the cache stale so the next iteration re-probes the changed repo.
func (p *loopProbe) current() map[string]string {
	m := p.refresh()
	p.primed = false
	return m
}
