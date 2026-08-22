package approvalrecordcontract

import "fmt"

var requestKeys = []string{
	"api_version", "approval_record", "canonicalization", "evaluated_at_unix_ms",
	"expected_target", "expected_target_sha256", "request_sha256",
}

func validateRequest(request map[string]any) error {
	if err := requireKeys(request, requestKeys...); err != nil {
		return fmt.Errorf("declared assessment request: %w", err)
	}
	api, apiErr := stringValue(request, "api_version")
	canonical, canonicalErr := stringValue(request, "canonicalization")
	if apiErr != nil || api != requestAPI || canonicalErr != nil || canonical != canonicalization {
		return fmt.Errorf("declared assessment request API/canonicalization is unsupported")
	}
	evaluated, err := intValue(request, "evaluated_at_unix_ms")
	if err != nil || evaluated < 0 {
		return fmt.Errorf("evaluated_at_unix_ms must be non-negative")
	}
	record, recordErr := objectValue(request, "approval_record")
	target, targetErr := objectValue(request, "expected_target")
	if recordErr != nil || targetErr != nil {
		return fmt.Errorf("approval_record and expected_target must be objects")
	}
	if err := validateRecord(record, false); err != nil {
		return err
	}
	if err := validateTarget(target); err != nil {
		return err
	}
	claimedTarget, targetHashErr := stringValue(request, "expected_target_sha256")
	computedTarget, digestErr := targetDigest(target)
	if targetHashErr != nil || validateHash(claimedTarget, "expected_target_sha256") != nil ||
		digestErr != nil || claimedTarget != computedTarget {
		return fmt.Errorf("expected_target_sha256 does not match expected_target")
	}
	if err := validateCanonicalByteLimit(request, maxRequestBytes, "declared assessment request"); err != nil {
		return err
	}
	return validateRequestDigest(request)
}

func validateRequestDigest(request map[string]any) error {
	claimed, err := stringValue(request, "request_sha256")
	if err != nil || validateHash(claimed, "request_sha256") != nil {
		return fmt.Errorf("request_sha256 is invalid")
	}
	preimage := cloneNode(request)
	preimage["request_sha256"] = ""
	computed, err := digestNode(requestDomain, preimage)
	if err != nil || computed != claimed {
		return fmt.Errorf("request_sha256 does not match canonical request")
	}
	return nil
}

func requestDigest(request map[string]any) (string, error) {
	preimage := cloneNode(request)
	preimage["request_sha256"] = ""
	return digestNode(requestDomain, preimage)
}
