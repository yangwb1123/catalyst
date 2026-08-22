package approvalrecordcontract

import "fmt"

func validateValidity(node map[string]any) error {
	if err := requireKeys(node, "expires_at_unix_ms", "issued_at_unix_ms", "not_before_unix_ms",
		"revoked_at_unix_ms", "transferable"); err != nil {
		return err
	}
	issued, issuedErr := intValue(node, "issued_at_unix_ms")
	starts, startsErr := intValue(node, "not_before_unix_ms")
	expires, expiresErr := intValue(node, "expires_at_unix_ms")
	if issuedErr != nil || startsErr != nil || expiresErr != nil || issued < 0 ||
		issued > starts || starts >= expires || expires-issued > maxTTLMillis {
		return fmt.Errorf("validity must be ordered within the 24-hour maximum window")
	}
	if node["revoked_at_unix_ms"] != nil {
		revoked, err := intValue(node, "revoked_at_unix_ms")
		if err != nil || revoked < issued || revoked >= expires {
			return fmt.Errorf("revoked_at_unix_ms must be null or inside issuance validity")
		}
	}
	transferable, err := boolValue(node, "transferable")
	if err != nil || transferable {
		return fmt.Errorf("ApprovalRecord must be non-transferable")
	}
	return nil
}
