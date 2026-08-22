package knowledgeupdateproposalcontract

import "fmt"

var requestKeys = []string{
	"api_version", "canonicalization", "evaluated_at_unix_ms", "expected_target",
	"expected_target_sha256", "knowledge_update_proposal", "request_sha256",
}

func validateRequest(request map[string]any) error {
	if err := validateCanonicalByteLimit(request, maxRequestBytes, "knowledge update declared assessment request"); err != nil {
		return err
	}
	if err := requireKeys(request, requestKeys...); err != nil {
		return fmt.Errorf("knowledge update declared assessment request: %w", err)
	}
	if err := requireStringLiteral(request, "api_version", requestAPI); err != nil {
		return err
	}
	if err := requireStringLiteral(request, "canonicalization", canonicalization); err != nil {
		return err
	}
	evaluated, err := intValue(request, "evaluated_at_unix_ms")
	if err != nil || evaluated < 0 {
		return fmt.Errorf("evaluated_at_unix_ms must be non-negative")
	}
	proposal, proposalErr := objectValue(request, "knowledge_update_proposal")
	target, targetErr := objectValue(request, "expected_target")
	if proposalErr != nil || targetErr != nil {
		return fmt.Errorf("knowledge_update_proposal and expected_target must be objects")
	}
	if err := validateProposal(proposal); err != nil {
		return err
	}
	if err := validateTarget(target); err != nil {
		return err
	}
	claimedTarget, err := stringValue(request, "expected_target_sha256")
	computedTarget, digestErr := targetDigest(target)
	if err != nil || validateHash(claimedTarget, "expected_target_sha256") != nil ||
		digestErr != nil || claimedTarget != computedTarget {
		return fmt.Errorf("expected_target_sha256 does not match expected_target")
	}
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
	if err := validateCanonicalByteLimit(preimage, maxRequestBytes, "assessment request digest preimage"); err != nil {
		return "", err
	}
	return digestValue(requestDomain, preimage)
}
