package transitionreceiptcontract

import "fmt"

var requestKeys = []string{
	"api_version", "canonicalization", "evaluated_at_unix_ms", "expected_target",
	"expected_target_sha256", "previous_receipt", "request_sha256", "transition_receipt",
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
	receipt, receiptErr := objectValue(request, "transition_receipt")
	target, targetErr := objectValue(request, "expected_target")
	if receiptErr != nil || targetErr != nil {
		return fmt.Errorf("transition_receipt and expected_target must be objects")
	}
	if err := validateReceipt(receipt, false); err != nil {
		return err
	}
	if err := validateTarget(target); err != nil {
		return err
	}
	if err := validateOptionalPrevious(request); err != nil {
		return err
	}
	if err := validateExpectedTargetDigest(request, target); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(request, maxRequestBytes, "declared assessment request"); err != nil {
		return err
	}
	return validateRequestDigest(request)
}

func validateOptionalPrevious(request map[string]any) error {
	if request["previous_receipt"] == nil {
		return nil
	}
	previous, ok := request["previous_receipt"].(map[string]any)
	if !ok {
		return fmt.Errorf("previous_receipt must be null or a TransitionReceipt")
	}
	return validateReceipt(previous, false)
}

func validateExpectedTargetDigest(request, target map[string]any) error {
	claimed, claimedErr := stringValue(request, "expected_target_sha256")
	computed, digestErr := targetDigest(target)
	if claimedErr != nil || validateHash(claimed, "expected_target_sha256") != nil ||
		digestErr != nil || claimed != computed {
		return fmt.Errorf("expected_target_sha256 does not match expected_target")
	}
	return nil
}

func validateRequestDigest(request map[string]any) error {
	claimed, err := stringValue(request, "request_sha256")
	if err != nil || validateHash(claimed, "request_sha256") != nil {
		return fmt.Errorf("request_sha256 is invalid")
	}
	computed, err := requestDigest(request)
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
