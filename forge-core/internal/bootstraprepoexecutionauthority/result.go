package bootstraprepoexecutionauthority

import (
	"encoding/base64"
	"fmt"
)

var resultKeys = []string{"api_version", "canonicalization", "completed_at_unix_ms", "content_bytes",
	"execution_policy_sha256", "execution_result_id", "execution_result_sha256", "execution_trust_epoch",
	"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id", "grant_sha256",
	"invocation_id", "invocation_sha256", "issuance_trust_epoch", "issuance_trust_root_sha256",
	"kind", "manifest_sha256", "observation_semantics", "observed_usage", "profile_id", "reads",
	"requested_action_sha256"}
var resultReadKeys = []string{"content_base64url", "content_bytes", "content_sha256", "path"}
var observedUsageKeys = []string{"call_count", "cost_usd_micros", "elapsed_ms", "input_tokens",
	"network_bytes", "output_bytes", "output_tokens"}

// Result is a strict first-delivery-only raw read result. It is never ledger data.
type Result struct{ document map[string]any }

// BuildResult validates ordered raw content against the exact Manifest.
func BuildResult(policy *Policy, invocation *Invocation, manifest *Manifest,
	contents [][]byte, completedAt, elapsedMillis int64) (*Result, error) {
	if policy == nil || invocation == nil || manifest == nil {
		return nil, fmt.Errorf("Policy, Invocation, and Manifest are required")
	}
	if completedAt < 0 {
		return nil, fmt.Errorf("completion time is negative")
	}
	reads, total, err := buildReads(manifest.document, contents)
	if err != nil {
		return nil, err
	}
	document := resultDocument(policy, invocation, manifest, reads, total,
		completedAt, elapsedMillis)
	digest, err := selfDigest(resultDomain, document, "execution_result_sha256",
		maxResultBytes, "BootstrapRepoReadExecutionResult", false, "execution_result_id")
	if err != nil {
		return nil, err
	}
	document["execution_result_sha256"] = digest
	document["execution_result_id"] = "bootstrap-repo-read-result-" + digest
	if err = validateResult(document, policy, invocation, manifest); err != nil {
		return nil, err
	}
	return &Result{document}, nil
}

func decodeResult(data []byte, policy *Policy, invocation *Invocation,
	manifest *Manifest) (*Result, error) {
	if policy == nil || invocation == nil || manifest == nil {
		return nil, fmt.Errorf("Policy, Invocation, and Manifest are required")
	}
	document, err := decodeCanonical(data, maxResultBytes)
	if err != nil {
		return nil, err
	}
	if err = validateResult(document, policy, invocation, manifest); err != nil {
		return nil, err
	}
	return &Result{document}, nil
}

func (result *Result) canonicalDocument() map[string]any { return cloneDocument(result.document) }

func buildReads(manifest map[string]any, contents [][]byte) ([]any, int64, error) {
	entries, _ := arrayValue(manifest, "entries")
	if len(contents) != len(entries) {
		return nil, 0, fmt.Errorf("raw content count differs from Manifest")
	}
	reads := make([]any, 0, len(entries))
	var total int64
	for index, entryValue := range entries {
		entry := entryValue.(map[string]any)
		content := contents[index]
		if int64(len(content)) != entry["content_bytes"] || plainDigest(content) != entry["content_sha256"] {
			return nil, 0, fmt.Errorf("raw content %d differs from Manifest", index)
		}
		total += int64(len(content))
		reads = append(reads, map[string]any{"content_base64url": base64.RawURLEncoding.EncodeToString(content),
			"content_bytes": int64(len(content)), "content_sha256": entry["content_sha256"],
			"path": entry["path"]})
	}
	return reads, total, nil
}

func resultDocument(policy *Policy, invocation *Invocation, manifest *Manifest,
	reads []any, total, completedAt, elapsed int64) map[string]any {
	request := invocation.document
	return map[string]any{"api_version": resultAPI, "canonicalization": canonicalization,
		"completed_at_unix_ms": completedAt, "content_bytes": total,
		"execution_policy_sha256": policy.document["execution_policy_sha256"],
		"execution_result_id":     "", "execution_result_sha256": "",
		"execution_trust_epoch":       request["execution_trust_epoch"],
		"execution_trust_root_sha256": request["execution_trust_root_sha256"],
		"grant_envelope_sha256":       request["grant_envelope_sha256"], "grant_id": request["grant_id"],
		"grant_sha256": request["grant_sha256"], "invocation_id": request["invocation_id"],
		"invocation_sha256": request["invocation_sha256"], "issuance_trust_epoch": request["issuance_trust_epoch"],
		"issuance_trust_root_sha256": request["issuance_trust_root_sha256"],
		"kind":                       "BootstrapRepoReadExecutionResult", "manifest_sha256": manifest.document["manifest_sha256"],
		"observation_semantics": "manifest_bound_ordered_non_atomic_raw_file_reads",
		"observed_usage": map[string]any{"call_count": int64(1), "cost_usd_micros": int64(0),
			"elapsed_ms": elapsed, "input_tokens": int64(0), "network_bytes": int64(0),
			"output_bytes": total, "output_tokens": int64(0)},
		"profile_id": profileID, "reads": reads,
		"requested_action_sha256": request["requested_action_sha256"]}
}

func validateResult(document map[string]any, policy *Policy, invocation *Invocation,
	manifest *Manifest) error {
	if err := requireKeys(document, resultKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadExecutionResult: %w", err)
	}
	if err := validateResultShape(document); err != nil {
		return err
	}
	if err := validateResultRelations(document, policy.document, invocation.document, manifest.document); err != nil {
		return err
	}
	claimed := document["execution_result_sha256"].(string)
	computed, err := selfDigest(resultDomain, document, "execution_result_sha256",
		maxResultBytes, "BootstrapRepoReadExecutionResult", false, "execution_result_id")
	if err != nil || claimed != computed {
		return fmt.Errorf("ExecutionResult self digest does not match")
	}
	if document["execution_result_id"] != "bootstrap-repo-read-result-"+claimed {
		return fmt.Errorf("ExecutionResult identity is invalid")
	}
	return nil
}

func validateResultShape(document map[string]any) error {
	if err := validateEnvelope(document, resultAPI, "BootstrapRepoReadExecutionResult"); err != nil {
		return err
	}
	if document["profile_id"] != profileID ||
		document["observation_semantics"] != "manifest_bound_ordered_non_atomic_raw_file_reads" {
		return fmt.Errorf("ExecutionResult profile or semantics is invalid")
	}
	for _, field := range []string{"execution_policy_sha256", "execution_result_sha256",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_sha256", "invocation_sha256",
		"issuance_trust_root_sha256", "manifest_sha256", "requested_action_sha256"} {
		if err := validateHashField(document, field, "ExecutionResult "+field); err != nil {
			return err
		}
	}
	if completed, err := intValue(document, "completed_at_unix_ms"); err != nil || completed < 0 {
		return fmt.Errorf("ExecutionResult completion time is invalid")
	}
	return nil
}

func validateResultRelations(result, policy, invocation, manifest map[string]any) error {
	for _, field := range []string{"execution_policy_sha256", "execution_trust_epoch",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id", "grant_sha256",
		"invocation_id", "invocation_sha256", "issuance_trust_epoch", "issuance_trust_root_sha256",
		"manifest_sha256", "profile_id", "requested_action_sha256"} {
		if !sameCanonical(result[field], invocation[field]) {
			return fmt.Errorf("ExecutionResult field %s differs from Invocation", field)
		}
	}
	if result["execution_policy_sha256"] != policy["execution_policy_sha256"] ||
		result["manifest_sha256"] != manifest["manifest_sha256"] {
		return fmt.Errorf("ExecutionResult Policy or Manifest binding is invalid")
	}
	return validateResultReads(result, invocation, manifest)
}

func validateResultReads(result, invocation, manifest map[string]any) error {
	reads, readsErr := arrayValue(result, "reads")
	entries, _ := arrayValue(manifest, "entries")
	total, totalErr := intValue(result, "content_bytes")
	if readsErr != nil || len(reads) != len(entries) || totalErr != nil || total < 0 || total > maxContentBytes {
		return fmt.Errorf("ExecutionResult reads or aggregate byte count is invalid")
	}
	var observed int64
	for index := range reads {
		count, err := validateResultRead(reads[index], entries[index], index)
		if err != nil {
			return err
		}
		observed += count
	}
	if observed != total {
		return fmt.Errorf("ExecutionResult aggregate byte count differs from reads")
	}
	return validateObservedUsage(result["observed_usage"], invocation, total)
}

func validateResultRead(value, expected any, index int) (int64, error) {
	read, ok := value.(map[string]any)
	entry, entryOK := expected.(map[string]any)
	if !ok || !entryOK || requireKeys(read, resultReadKeys...) != nil {
		return 0, fmt.Errorf("ExecutionResult read %d shape is invalid", index)
	}
	content, err := stringValue(read, "content_base64url")
	decoded, decodeErr := decodeBase64URL(content, "read content", -1)
	count, countErr := intValue(read, "content_bytes")
	if err != nil || decodeErr != nil || countErr != nil || count != int64(len(decoded)) ||
		count != entry["content_bytes"] || read["path"] != entry["path"] ||
		read["content_sha256"] != entry["content_sha256"] || plainDigest(decoded) != entry["content_sha256"] {
		return 0, fmt.Errorf("ExecutionResult read %d differs from Manifest", index)
	}
	return count, nil
}

func validateObservedUsage(value any, invocation map[string]any, total int64) error {
	usage, ok := value.(map[string]any)
	if !ok || requireKeys(usage, observedUsageKeys...) != nil || usage["call_count"] != int64(1) ||
		usage["cost_usd_micros"] != int64(0) || usage["input_tokens"] != int64(0) ||
		usage["network_bytes"] != int64(0) || usage["output_bytes"] != total ||
		usage["output_tokens"] != int64(0) {
		return fmt.Errorf("ExecutionResult observed_usage is invalid")
	}
	elapsed, elapsedErr := intValue(usage, "elapsed_ms")
	action := invocation["requested_action"].(map[string]any)
	requestedUsage := action["usage"].(map[string]any)
	timeout, _ := intValue(requestedUsage, "timeout_ms")
	if elapsedErr != nil || elapsed < 0 || elapsed > timeout {
		return fmt.Errorf("ExecutionResult cooperative elapsed_ms exceeds budget")
	}
	return nil
}
