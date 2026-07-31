package asset

import (
	"fmt"
	"strings"
)

// ScanContractEvolveV1 is the strict Evolve scan report protocol.
const ScanContractEvolveV1 = "evolve_scan_v1"

// validateScanContracts pins the capability shape without relying on a phase
// name or agent role. A stage-less Workflow is accepted because the dependency
// planner validates phase-only projections; full workflow loads still enforce
// stage=evolve.
func validateScanContracts(wf Workflow) error {
	declared := ""
	for index, phase := range wf.Phases {
		if phase.ScanContract == "" {
			continue
		}
		if phase.ScanContract != ScanContractEvolveV1 {
			return fmt.Errorf("asset: phase %q has unsupported scan_contract %q",
				phase.Name, phase.ScanContract)
		}
		if declared != "" {
			return fmt.Errorf("asset: Evolve scan_contract %q is declared by both %q and %q",
				ScanContractEvolveV1, declared, phase.Name)
		}
		declared = phase.Name
		if index != 0 {
			return fmt.Errorf("asset: phase %q scan_contract %q must be the first phase so every later phase observes its validated report",
				phase.Name, phase.ScanContract)
		}
		if wf.Stage != "" && wf.Stage != "evolve" || !phase.Readonly || phase.Effect != "observe" {
			return fmt.Errorf("asset: phase %q scan_contract %q requires stage evolve, readonly=true, and effect=observe",
				phase.Name, phase.ScanContract)
		}
		if phase.Agent == "harness" || len(phase.RequiredGates) > 0 {
			return fmt.Errorf("asset: phase %q scan_contract %q must execute a non-harness Agent with required_gates=[]",
				phase.Name, phase.ScanContract)
		}
		if len(phase.DependsOn) > 0 {
			return fmt.Errorf("asset: phase %q scan_contract %q is the root producer and requires depends_on=[]",
				phase.Name, phase.ScanContract)
		}
		if len(phase.Emits) > 0 || phase.WritesADR != nil {
			return fmt.Errorf("asset: phase %q scan_contract %q must not grant emits or writes_adr",
				phase.Name, phase.ScanContract)
		}
		if !phase.FeedsForward {
			return fmt.Errorf("asset: phase %q scan_contract %q requires feeds_forward=true",
				phase.Name, phase.ScanContract)
		}
		if strings.TrimSpace(phase.RequiredWhen) != "" || len(phase.OptionalFor) > 0 {
			return fmt.Errorf("asset: phase %q scan_contract %q must not be mode-skippable",
				phase.Name, phase.ScanContract)
		}
	}
	return validateScanDependencies(wf, declared)
}

// validateScanDependencies keeps the optional parallel graph honest. With no
// dependencies the CLI stays serial; once any edge opts in, every later phase
// must be downstream of the scan producer.
func validateScanDependencies(wf Workflow, scanName string) error {
	if scanName == "" || !workflowDeclaresDependencies(wf) {
		return nil
	}
	byName := make(map[string]Phase, len(wf.Phases))
	for _, phase := range wf.Phases {
		byName[phase.Name] = phase
	}
	memo := map[string]bool{}
	var reachesScan func(string, map[string]bool) bool
	reachesScan = func(name string, active map[string]bool) bool {
		if result, ok := memo[name]; ok {
			return result
		}
		if active[name] {
			return false
		}
		next := make(map[string]bool, len(active)+1)
		for key := range active {
			next[key] = true
		}
		next[name] = true
		for _, dependency := range byName[name].DependsOn {
			if dependency == scanName || reachesScan(dependency, next) {
				memo[name] = true
				return true
			}
		}
		memo[name] = false
		return false
	}
	for _, phase := range wf.Phases[1:] {
		if !reachesScan(phase.Name, nil) {
			return fmt.Errorf("asset: phase %q must transitively depend on contracted scan phase %q when depends_on enables parallel mode",
				phase.Name, scanName)
		}
	}
	return nil
}

func workflowDeclaresDependencies(wf Workflow) bool {
	for _, phase := range wf.Phases {
		if len(phase.DependsOn) > 0 {
			return true
		}
	}
	return false
}
