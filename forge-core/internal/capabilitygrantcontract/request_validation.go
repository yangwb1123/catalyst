package capabilitygrantcontract

import "fmt"

var requestKeys = []string{
	"api_version", "canonicalization", "evaluated_at_unix_ms", "expected", "grant", "request_sha256",
	"requested_action",
}

func validateAssessmentRequest(request map[string]any) error {
	if err := requireKeys(request, requestKeys...); err != nil {
		return fmt.Errorf("assessment request: %w", err)
	}
	if err := requireStringLiteral(request, "api_version",
		"forgeos.capability-grant-declared-assessment-request/v1"); err != nil {
		return err
	}
	if err := requireStringLiteral(request, "canonicalization", "forgeos.canonical-json/v1"); err != nil {
		return err
	}
	evaluated, err := intValue(request, "evaluated_at_unix_ms")
	if err != nil || evaluated < 0 {
		return fmt.Errorf("evaluated_at_unix_ms must be non-negative")
	}
	grant, grantErr := objectValue(request, "grant")
	expected, expectedErr := objectValue(request, "expected")
	action, actionErr := objectValue(request, "requested_action")
	if grantErr != nil || expectedErr != nil || actionErr != nil {
		return fmt.Errorf("grant, expected, and requested_action must be objects")
	}
	if err := validateGrant(grant); err != nil {
		return err
	}
	if err := validateExpected(expected); err != nil {
		return err
	}
	if err := validateRequestedAction(action); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(request, maxAssessmentRequestBytes, "assessment request"); err != nil {
		return err
	}
	return validateRequestDigest(request)
}

func validateExpected(expected map[string]any) error {
	if err := requireKeys(expected, "bindings", "capability", "subject", "task_binding"); err != nil {
		return err
	}
	validators := map[string]func(map[string]any) error{
		"bindings": validateBindings, "capability": validateCapability,
		"subject": validatePrincipal, "task_binding": validateTaskBinding,
	}
	for key, validator := range validators {
		node, err := objectValue(expected, key)
		if err != nil {
			return err
		}
		if err := validator(node); err != nil {
			return fmt.Errorf("expected %s: %w", key, err)
		}
	}
	return nil
}

func validateRequestedAction(action map[string]any) error {
	if err := requireKeys(action, "effect_id", "resources", "usage"); err != nil {
		return err
	}
	effectID, err := stringValue(action, "effect_id")
	if err != nil {
		return err
	}
	descriptor, err := findEffect(effectID)
	if err != nil {
		return err
	}
	resources, err := arrayValue(action, "resources")
	if err != nil || len(resources) < 1 || len(resources) > 32 {
		return fmt.Errorf("requested action resources must contain 1..32 items")
	}
	if err := validateProfileResources(resources, descriptor, true); err != nil {
		return fmt.Errorf("requested action resources: %w", err)
	}
	usage, err := objectValue(action, "usage")
	if err != nil {
		return err
	}
	if err := validateUsage(usage); err != nil {
		return err
	}
	return validateCommandUsageConsistency(effectID, resources, usage)
}

func validateCommandUsageConsistency(effectID string, resources []any, usage map[string]any) error {
	if effectID != "process.exec" {
		return nil
	}
	command := resources[0].(map[string]any)
	commandTimeout, _ := intValue(command, "timeout_ms")
	usageTimeout, _ := intValue(usage, "timeout_ms")
	if commandTimeout != usageTimeout {
		return fmt.Errorf("process.exec command timeout_ms must equal requested usage timeout_ms")
	}
	return nil
}

func validateUsage(usage map[string]any) error {
	keys := []string{"call_count", "cost_usd_micros", "input_tokens", "network_bytes", "output_bytes",
		"output_tokens", "timeout_ms"}
	if err := requireKeys(usage, keys...); err != nil {
		return err
	}
	bounds := map[string]int64{
		"call_count": 1000000000, "cost_usd_micros": 1000000000000000,
		"input_tokens": 1000000000, "network_bytes": 1073741824,
		"output_bytes": 1073741824, "output_tokens": 1000000000, "timeout_ms": 86400000,
	}
	for _, key := range keys {
		value, err := intValue(usage, key)
		minimum := int64(0)
		if key == "call_count" || key == "timeout_ms" {
			minimum = 1
		}
		if err != nil || value < minimum || value > bounds[key] {
			return fmt.Errorf("usage %s is outside its v1 bound", key)
		}
	}
	return nil
}

func validateRequestDigest(request map[string]any) error {
	claimed, err := stringValue(request, "request_sha256")
	if err != nil || validateHash(claimed, "request_sha256") != nil {
		return fmt.Errorf("request_sha256 is invalid")
	}
	preimage := cloneNode(request)
	preimage["request_sha256"] = ""
	computed, err := digestNode(requestDigestDomain, preimage)
	if err != nil || computed != claimed {
		return fmt.Errorf("request_sha256 does not match canonical request")
	}
	return nil
}

func requestedActionDigest(action map[string]any) (string, error) {
	return digestNode(actionDigestDomain, action)
}
