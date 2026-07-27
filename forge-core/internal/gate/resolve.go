// resolve.go — the lifecycle-aware N/A exemption matrix and the per-gate
// resolution logic: turning a logical gate NAME (lint/build/test/security/
// arch/complexity/…) into its real tri-state Result, honoring the
// two-dimensional (status × category) waiver rules. This is pure business
// logic over the acceptance probe map — no CLI concerns — which is why it
// lives in this package (extending gate.go's ProbeAll/Result) rather than in
// cmd/forge; see internal/attribution, internal/doctor, internal/migrate,
// internal/mode, internal/risk for the same "keep cmd/forge a thin CLI-
// dispatch layer" pattern. cmd/forge calls in via GatesGreen/ResolveGate/
// HarnessRunner; everything else here is an internal implementation detail.
package gate

import (
	"fmt"

	"forgeos/forge-core/internal/converge"
)

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
// rules in GatesGreen).
func exemptsNoTool(lifecycle string) bool {
	switch lifecycle {
	case "idea", "mvp", "growth":
		return true
	default: // production, unknown, "" -> strict
		return false
	}
}

// GatesGreen is the lifecycle-aware convergence judgment over the required gates,
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
func GatesGreen(root string, names []string, probe, categories map[string]string, lifecycle string) (bool, converge.GateProof) {
	var proof converge.GateProof
	if len(names) == 0 {
		return false, proof
	}
	provenCount := 0 // non-NA gates that PASSED (the vacuous-green guard's numerator)
	green := true
	for _, name := range names {
		res := ResolveGate(root, name, probe)
		switch res.Status {
		case StatusPass:
			provenCount++
			proof.Proven = append(proof.Proven, name)
		case StatusNA:
			cat := gateCategory(name, probe, categories)
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

// gateCategory resolves the N/A category for a logical gate. Composite gates
// use the first underlying criterion that is absent/N/A, so a broken required
// probe cannot borrow a more permissive sibling's exemption.
func gateCategory(name string, probe, categories map[string]string) string {
	switch name {
	case "security":
		return firstNACategory(probe, categories, "security_findings", "dependency_vulnerabilities")
	case "test":
		return firstNACategory(probe, categories, "test_pass", "app_test_pass")
	default:
		return categories[name]
	}
}

func firstNACategory(probe, categories map[string]string, criteria ...string) string {
	for _, criterion := range criteria {
		if status, ok := probe[criterion]; !ok || status == StatusNA {
			return categories[criterion]
		}
	}
	return ""
}

// naReason renders a short, honest reason for a waived N/A gate, preferring the
// acceptance probe's own detail and falling back to the category.
func naReason(res Result, category string) string {
	if res.Output != "" {
		return res.Output
	}
	return category
}

// HarnessRunner maps logical gate names onto their REAL per-gate verdict, using
// the once-per-run acceptance probe map. It is the fix for the FAKE PASS: each
// required gate resolves to its own PASS/FAIL/NA instead of every name sharing
// one coarse aggregate verdict.
func HarnessRunner(repoRoot string, probe map[string]string) func(string) Result {
	return func(name string) Result {
		return ResolveGate(repoRoot, name, probe)
	}
}

// ResolveGate computes one gate's honest tri-state Result. Structural gates run
// their own tool live; the rest read the shared acceptance probe:
//
//	complexity -> Gate  (structural caps, a real check)
//	arch       -> Check (governance integrity, a real check)
//	test       -> PASS iff BOTH acceptance test_pass AND app_test_pass are PASS
//	lint       -> acceptance 'lint'            (N/A here: no linter installed)
//	build      -> acceptance 'build'           (N/A here: no build step)
//	security   -> BOTH acceptance 'security_findings' and
//	              'dependency_vulnerabilities' (secret scan + SCA)
//
// An acceptance-backed gate whose criterion is missing from the probe map (or
// whose probe failed) resolves to N/A — honest "not checked", never a pass.
func ResolveGate(repoRoot, name string, probe map[string]string) Result {
	switch name {
	case "complexity":
		return Gate(repoRoot)
	case "arch":
		return Check(repoRoot)
	case "test":
		return combinedGate(name, probe, "test_pass", "app_test_pass")
	case "lint":
		return probedGate(name, probe, "lint")
	case "build":
		return probedGate(name, probe, "build")
	case "security":
		return combinedGate(name, probe, "security_findings", "dependency_vulnerabilities")
	default:
		return probedGate(name, probe, name)
	}
}

// probedGate reads one acceptance criterion's status from the probe map and
// renders it as a tri-state Result. A criterion absent from the map means
// the probe did not report it (or did not run) -> N/A, never a silent pass.
func probedGate(name string, probe map[string]string, criterion string) Result {
	status, ok := probe[criterion]
	if !ok {
		status = StatusNA
	}
	return Result{
		Name:   name,
		Status: status,
		OK:     status == StatusPass,
		Output: probeDetail(criterion, status),
	}
}

// combinedGate ANDs several acceptance criteria into one gate. It is PASS only
// when every input criterion is PASS; if any input is N/A (and none FAIL) the
// gate is N/A; any FAIL makes it FAIL. This is how `test` requires BOTH the
// harness suites (test_pass) and the dogfood app suite (app_test_pass).
func combinedGate(name string, probe map[string]string, criteria ...string) Result {
	status := StatusPass
	for _, c := range criteria {
		switch probe[c] {
		case StatusFail:
			status = StatusFail
		case StatusPass:
			// keep current (PASS unless already downgraded)
		default: // absent or NA
			if status != StatusFail {
				status = StatusNA
			}
		}
	}
	return Result{
		Name:   name,
		Status: status,
		OK:     status == StatusPass,
		Output: fmt.Sprintf("%v -> %s", criteria, status),
	}
}

// probeDetail renders a short, honest reason line for one resolved criterion.
func probeDetail(criterion, status string) string {
	switch status {
	case StatusNA:
		return fmt.Sprintf("no executable check for %q in this repo", criterion)
	case StatusFail:
		return fmt.Sprintf("acceptance criterion %q failed", criterion)
	default:
		return fmt.Sprintf("acceptance criterion %q passed", criterion)
	}
}
