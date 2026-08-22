package authenticatedadrapprovalcontract

import (
	"bytes"
	"fmt"
)

type objectValidator func(any) (map[string]any, error)

func decodeCanonicalObject(raw []byte, maximum int, label string,
	validator objectValidator) (map[string]any, error) {
	value, err := parseStrictJSON(raw, maximum)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", label, err)
	}
	node, err := validator(value)
	if err != nil {
		return nil, err
	}
	canonical, err := boundedCanonicalJSON(node, maximum, label)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("%s is not exact compact canonical JSON", label)
	}
	return node, nil
}

// DecodeCanonicalBundle strictly decodes and validates one candidate bundle.
func DecodeCanonicalBundle(raw []byte) (*Bundle, error) {
	value, err := parseStrictJSON(raw, maxBundleBytes)
	if err != nil {
		return nil, err
	}
	node, root, err := validateBundle(value)
	if err != nil {
		return nil, err
	}
	canonical, err := boundedCanonicalJSON(node, maxBundleBytes, "ADR approval candidate bundle")
	if err != nil || !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("ADR approval candidate bundle is not exact compact canonical JSON")
	}
	rootCopy := &TrustRoot{document: cloneValue(root).(map[string]any)}
	return &Bundle{document: cloneValue(node).(map[string]any), root: rootCopy}, nil
}

// CanonicalBundleJSON returns exact validated instance bytes without a trailing LF.
func CanonicalBundleJSON(bundle *Bundle) ([]byte, error) {
	if bundle == nil {
		return nil, fmt.Errorf("bundle is nil")
	}
	node, _, err := validateBundle(bundle.document)
	if err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(node, maxBundleBytes, "ADR approval candidate bundle")
}

// DecodeCanonicalTrustRoot strictly decodes a root bound to the frozen profile.
func DecodeCanonicalTrustRoot(raw []byte) (*TrustRoot, error) {
	node, err := decodeCanonicalObject(raw, maxRootBytes, "ArchitectureDecisionApprovalTrustRoot",
		func(value any) (map[string]any, error) {
			return validateTrustRoot(value, signatureProfileSHA256Pin)
		})
	if err != nil {
		return nil, err
	}
	return &TrustRoot{document: cloneValue(node).(map[string]any)}, nil
}

// DecodeCanonicalLedger strictly decodes a complete caller-supplied ledger.
func DecodeCanonicalLedger(raw []byte, root *TrustRoot) (*Ledger, error) {
	if root == nil {
		return nil, fmt.Errorf("trust root is nil")
	}
	node, err := decodeCanonicalObject(raw, maxLedgerBytes,
		"ArchitectureDecisionApprovalAuthorizationLedger", func(value any) (map[string]any, error) {
			return validateLedger(value, root.document)
		})
	if err != nil {
		return nil, err
	}
	return &Ledger{document: cloneValue(node).(map[string]any), root: root}, nil
}

// DecodeCanonicalReceipt strictly decodes a receipt bound to the supplied
// approval trust root. The returned receipt is structural until its detached
// SignatureChecks are verified by an authority with an external root pin.
func DecodeCanonicalReceipt(raw []byte, root *TrustRoot) (*Receipt, error) {
	if root == nil {
		return nil, fmt.Errorf("trust root is nil")
	}
	node, err := decodeCanonicalObject(raw, maxReceiptBytes,
		"ArchitectureDecisionApprovalAuthorizationReceipt", func(value any) (map[string]any, error) {
			return validateReceipt(value, root.document)
		})
	if err != nil {
		return nil, err
	}
	return &Receipt{document: cloneValue(node).(map[string]any), root: root}, nil
}

// CanonicalLedgerJSON returns exact validated ledger bytes without a trailing LF.
func CanonicalLedgerJSON(ledger *Ledger) ([]byte, error) {
	if ledger == nil || ledger.root == nil {
		return nil, fmt.Errorf("ledger or trust root is nil")
	}
	node, err := validateLedger(ledger.document, ledger.root.document)
	if err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(node, maxLedgerBytes,
		"ArchitectureDecisionApprovalAuthorizationLedger")
}

// CanonicalResultJSON projects and validates the result nested in a bundle.
func CanonicalResultJSON(bundle *Bundle) ([]byte, error) {
	if bundle == nil || bundle.root == nil {
		return nil, fmt.Errorf("bundle or trust root is nil")
	}
	result, err := validateResult(bundle.document["authorization_result"], bundle.root.document)
	if err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(result, maxResultBytes,
		"ArchitectureDecisionApprovalAuthorizationResult")
}
