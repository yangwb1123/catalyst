package orchestrator

import (
	"fmt"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
)

// runGates resolves every required gate of a phase with three honest outcomes:
//
//	PASS — the gate was actually CHECKED and passed: log "gate X ok", continue.
//	FAIL — a real check failed: log + ABORT the run (a red gate blocks the
//	       increment — enforcement).
//	NA   — no executable check backs this gate in THIS repo (e.g. lint/build/
//	       security with no tooling): log "gate X N/A (not checked: <detail>)"
//	       and continue. N/A is a known environmental limitation, NOT a pass and
//	       NOT a fail — it never counts as "ok" and never aborts the run.
//
// This is the fix for the FAKE PASS: never-checked gates used to be reported as
// "ok"; now they surface as N/A so the honesty of acceptance.mjs is preserved.
//
// gates is the mode-FILTERED gate list (required_gates ∩ ModePolicy.Gates, or
// the full required_gates when gating is inactive) computed by gatesFor — Run
// passes it in so the filtering lives in one place. An empty gates slice is a
// legal no-op: the phase runs no gate under this mode (logged for visibility).
func (e Engine) runGates(p asset.Phase, gates []string) error {
	if len(gates) < len(p.RequiredGates) {
		e.logf("phase %s: mode gating runs %d/%d gates (%v)", p.Name, len(gates), len(p.RequiredGates), gates)
	}
	for _, name := range gates {
		res := e.callGate(name)
		switch gateStatus(res) {
		case gate.StatusFail:
			e.logf("phase %s: gate %s FAILED", p.Name, name)
			e.onGateResult(name, "FAILED")
			return fmt.Errorf("phase %s: required gate %q not OK: %s", p.Name, name, res.Output)
		case gate.StatusNA:
			e.logf("phase %s: gate %s N/A (not checked: %s)", p.Name, name, naDetail(res))
			e.onGateResult(name, "N/A")
		default: // StatusPass
			e.logf("phase %s: gate %s ok", p.Name, name)
			e.onGateResult(name, "ok")
		}
	}
	return nil
}

// gateStatus reads a Result's tri-state Status, falling back to its OK flag when
// a runner supplies no explicit Status (back-compat: legacy/test fakes set only
// OK). This keeps OK==true -> PASS, OK==false -> FAIL, while honoring an
// explicit NA that a tri-state runner sets.
func gateStatus(res gate.Result) string {
	if res.Status != "" {
		return res.Status
	}
	if res.OK {
		return gate.StatusPass
	}
	return gate.StatusFail
}

// naDetail returns a short reason for an N/A gate, defaulting when the runner
// supplied no detail so the log line is always informative.
func naDetail(res gate.Result) string {
	if res.Output != "" {
		return res.Output
	}
	return "no executable check in this repo"
}

// callGate invokes the injected RunGate, or returns a failing result when none
// is wired so a missing dependency cannot masquerade as a pass.
func (e Engine) callGate(name string) gate.Result {
	if e.RunGate == nil {
		return gate.Result{Name: name, OK: false, Output: "no gate runner configured"}
	}
	return e.RunGate(name)
}

// onGateResult reports one gate's verdict to the OPTIONAL OnGateResult callback — the nil-safe mirror of logf (a no-op when unwired, so back-compat holds).
func (e Engine) onGateResult(name, status string) {
	if e.OnGateResult != nil {
		e.OnGateResult(name, status)
	}
}
