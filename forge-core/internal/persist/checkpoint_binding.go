package persist

import (
	"fmt"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/materiality"
)

func prepareCheckpointRecoveryMaps(cp *Checkpoint) {
	if cp.RunID == "" && cp.Materiality == materiality.Unbound {
		cp.RunID = "run_id_not_bound"
	}
	if cp.PhaseReceipts == nil {
		cp.PhaseReceipts = map[string]string{}
	}
	if cp.PhaseSemanticOutputs == nil {
		cp.PhaseSemanticOutputs = map[string]string{}
	}
	if cp.StageReceipts == nil {
		cp.StageReceipts = map[string]string{}
	}
	if cp.ApprovalContexts == nil {
		cp.ApprovalContexts = map[string]string{}
	}
}

func validateCheckpointRecoveryBindings(cp Checkpoint) error {
	if strings.TrimSpace(cp.RunID) == "" {
		return fmt.Errorf("persist: checkpoint %s required field %q must be non-empty",
			CheckpointFormatCurrent, "run_id")
	}
	if cp.ReceiptHead != "" && !validRecoveryDigest(cp.ReceiptHead) {
		return fmt.Errorf("persist: checkpoint receipt head must be empty or lowercase SHA-256")
	}
	checks := []struct {
		name   string
		values map[string]string
		phase  bool
	}{
		{"phase_output_receipts", cp.PhaseReceipts, true},
		{"stage_output_receipts", cp.StageReceipts, false},
		{"stage_approval_contexts", cp.ApprovalContexts, false},
	}
	if cp.PhaseSemanticOutputs == nil {
		return fmt.Errorf("persist: checkpoint phase_semantic_outputs must be a non-null object")
	}
	for key := range cp.PhaseSemanticOutputs {
		if !validRecoveryKey(key, true) {
			return fmt.Errorf("persist: checkpoint phase_semantic_outputs contains invalid key %q", key)
		}
	}
	for _, check := range checks {
		if check.values == nil {
			return fmt.Errorf("persist: checkpoint %s must be a non-null object", check.name)
		}
		for key, digest := range check.values {
			if !validRecoveryKey(key, check.phase) || !validRecoveryDigest(digest) {
				return fmt.Errorf("persist: checkpoint %s contains invalid reference %q", check.name, key)
			}
		}
	}
	if cp.ReceiptHead == "" &&
		(len(cp.PhaseReceipts) != 0 || len(cp.StageReceipts) != 0 || len(cp.ApprovalContexts) != 0) {
		return fmt.Errorf("persist: checkpoint recovery references require a receipt journal head")
	}
	return nil
}

func validateCheckpointScanOutputs(cp Checkpoint) error {
	if len(cp.EvolveScanReport) > checkpointScanReportMaxBytes ||
		!utf8.ValidString(cp.EvolveScanReport) ||
		len(cp.EvolveScanSemanticOutput) > checkpointScanReportMaxBytes ||
		!utf8.ValidString(cp.EvolveScanSemanticOutput) {
		return fmt.Errorf("persist: checkpoint scan outputs must be valid UTF-8 with at most %d bytes",
			checkpointScanReportMaxBytes)
	}
	if (cp.EvolveScanReport == "") != (cp.EvolveScanSemanticOutput == "") {
		return fmt.Errorf("persist: checkpoint scan report and semantic output must be present together")
	}
	if cp.PhaseIndex == 0 && cp.EvolveScanReport != "" {
		return fmt.Errorf("persist: checkpoint scan outputs require a positive phase_index")
	}
	for key, output := range cp.PhaseSemanticOutputs {
		if !utf8.ValidString(output) || len(output) > checkpointScanReportMaxBytes {
			return fmt.Errorf("persist: checkpoint semantic output %q exceeds bound or is invalid UTF-8", key)
		}
	}
	return nil
}

func validRecoveryKey(value string, phase bool) bool {
	if value == "" || strings.TrimSpace(value) != value || strings.ContainsAny(value, "\\\x00\r\n\t") {
		return false
	}
	if !phase {
		return !strings.Contains(value, "/")
	}
	stage, name, ok := strings.Cut(value, "/")
	return ok && stage != "" && name != "" && !strings.Contains(name, "/")
}

func validRecoveryDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}
