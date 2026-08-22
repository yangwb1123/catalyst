package authenticatedadrlifecyclecontract

import (
	"bytes"
	"fmt"
)

// DecodeCanonicalBundle strictly decodes one canonical structural bundle.
// A trailing LF is invalid in production instance mode.
func DecodeCanonicalBundle(raw []byte) (*Bundle, error) {
	value, err := parseStrictJSON(raw, maxGoldenBytes)
	if err != nil {
		return nil, err
	}
	node, context, err := validateDocument(value)
	if err != nil {
		return nil, err
	}
	canonical, err := boundedCanonicalJSON(node, maxGoldenBytes,
		"ADR lifecycle candidate bundle")
	if err != nil || !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("ADR lifecycle candidate bundle is not exact compact canonical JSON")
	}
	return bundleFromValidated(node, context), nil
}

// ValidateBundle revalidates an opaque bundle without producing authority.
func ValidateBundle(bundle *Bundle) error {
	if bundle == nil {
		return fmt.Errorf("bundle is nil")
	}
	_, _, err := validateDocument(bundle.document)
	return err
}

// CanonicalBundleJSON returns validated instance bytes without a trailing LF.
func CanonicalBundleJSON(bundle *Bundle) ([]byte, error) {
	if err := ValidateBundle(bundle); err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(bundle.document, maxGoldenBytes,
		"ADR lifecycle candidate bundle")
}

// CanonicalLedgerJSON returns the exact validated complete ledger projection.
func CanonicalLedgerJSON(bundle *Bundle) ([]byte, error) {
	return canonicalBundleProjection(bundle, "ledger", maxLedgerBytes)
}

// CanonicalMaterializedViewJSON returns the exact validated materialized view.
func CanonicalMaterializedViewJSON(bundle *Bundle) ([]byte, error) {
	return canonicalBundleProjection(bundle, "materialized_view", maxViewBytes)
}

// CanonicalStateJSON returns the exact validated enclosing state image.
func CanonicalStateJSON(bundle *Bundle) ([]byte, error) {
	return canonicalBundleProjection(bundle, "state", maxStateBytes)
}

// CanonicalResultJSON returns the exact validated unsigned result projection.
func CanonicalResultJSON(bundle *Bundle) ([]byte, error) {
	return canonicalBundleProjection(bundle, "result", maxResultBytes)
}

func canonicalBundleProjection(bundle *Bundle, kind string, maximum int) ([]byte, error) {
	if err := ValidateBundle(bundle); err != nil {
		return nil, err
	}
	state := bundle.document["lifecycle_state"].(map[string]any)
	var value any
	switch kind {
	case "ledger":
		value = state["ledger"]
	case "materialized_view":
		value = state["materialized_view"]
	case "state":
		value = state
	case "result":
		value = bundle.document["lifecycle_result"]
	default:
		return nil, fmt.Errorf("unsupported bundle projection %q", kind)
	}
	return boundedCanonicalJSON(value, maximum, "ADR lifecycle "+kind)
}

func bundleFromValidated(node map[string]any, context validationContext) *Bundle {
	return &Bundle{document: cloneValue(node).(map[string]any),
		approvalRoot:  context.approvalRoot,
		lifecycleRoot: cloneValue(context.lifecycleRoot).(map[string]any),
		profileHash:   context.profileHash}
}
