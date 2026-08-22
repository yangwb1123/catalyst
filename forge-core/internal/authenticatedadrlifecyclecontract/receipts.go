package authenticatedadrlifecyclecontract

import "fmt"

func validateAcceptance(value any, profileHash string,
	root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionLifecycleAcceptanceReceipt"
	fields := []string{"acceptance_id", "acceptance_sha256", "accepted_at_unix_ms", "adr_id",
		"api_version", "authorization_receipt_physical_sha256", "authorization_receipt_sha256",
		"canonicalization", "kind", "ledger_sequence", "profile_id",
		"proposal_binding_sha256", "record_key_sha256", "request_sha256", "signature",
		"supersedes", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxAcceptanceBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != acceptanceAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validateAcceptanceScalars(node); err != nil {
		return nil, err
	}
	if err = validateStateSignature(node, profileHash, root, "acceptance"); err != nil {
		return nil, err
	}
	digest, err := acceptanceSHA256(node)
	if err != nil || node["acceptance_sha256"] != digest {
		return nil, fmt.Errorf("acceptance receipt self digest does not match")
	}
	if node["acceptance_id"] != "architecture-decision-acceptance-"+digest {
		return nil, fmt.Errorf("acceptance ID does not match its receipt digest")
	}
	return node, nil
}

func validateAcceptanceScalars(node map[string]any) error {
	if _, err := adrIDValue(node["adr_id"], "acceptance.adr_id"); err != nil {
		return err
	}
	for field, minimum := range map[string]int64{"accepted_at_unix_ms": 0,
		"ledger_sequence": 1, "trust_epoch": 1} {
		if _, err := intValue(node[field], "acceptance."+field, minimum, maxInt64); err != nil {
			return err
		}
	}
	for _, field := range []string{"acceptance_sha256", "authorization_receipt_physical_sha256",
		"authorization_receipt_sha256", "proposal_binding_sha256", "record_key_sha256",
		"request_sha256", "trust_root_sha256"} {
		if _, err := shaValue(node[field], "acceptance."+field); err != nil {
			return err
		}
	}
	supersedes, err := sortedUniqueStrings(node["supersedes"], "acceptance.supersedes", 0, maxSupersessions)
	if err != nil {
		return err
	}
	for index, item := range supersedes {
		if _, err = adrIDValue(item, fmt.Sprintf("acceptance.supersedes[%d]", index)); err != nil {
			return err
		}
	}
	return nil
}

func validateSupersession(value any, profileHash string,
	root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionLifecycleSupersessionReceipt"
	fields := []string{"api_version", "canonicalization", "kind", "ledger_sequence", "profile_id",
		"receipt_id", "receipt_sha256", "request_sha256", "signature", "superseded_at_unix_ms",
		"superseded_by_acceptance_id", "superseded_by_adr_id",
		"superseded_by_proposal_binding_sha256", "target_acceptance_id", "target_adr_id",
		"target_proposal_binding_sha256", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxSupersessionBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != supersessionAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validateSupersessionScalars(node); err != nil {
		return nil, err
	}
	if err = validateStateSignature(node, profileHash, root, "supersession"); err != nil {
		return nil, err
	}
	digest, err := supersessionSHA256(node)
	if err != nil || node["receipt_sha256"] != digest {
		return nil, fmt.Errorf("supersession receipt self digest does not match")
	}
	if node["receipt_id"] != "architecture-decision-supersession-"+digest {
		return nil, fmt.Errorf("supersession receipt ID does not match its digest")
	}
	return node, nil
}

func validateSupersessionScalars(node map[string]any) error {
	for _, field := range []string{"superseded_by_adr_id", "target_adr_id"} {
		if _, err := adrIDValue(node[field], "supersession."+field); err != nil {
			return err
		}
	}
	for field, minimum := range map[string]int64{"ledger_sequence": 1,
		"superseded_at_unix_ms": 0, "trust_epoch": 1} {
		if _, err := intValue(node[field], "supersession."+field, minimum, maxInt64); err != nil {
			return err
		}
	}
	for _, field := range []string{"receipt_sha256", "request_sha256",
		"superseded_by_proposal_binding_sha256", "target_proposal_binding_sha256",
		"trust_root_sha256"} {
		if _, err := shaValue(node[field], "supersession."+field); err != nil {
			return err
		}
	}
	for _, field := range []string{"superseded_by_acceptance_id", "target_acceptance_id"} {
		if _, err := textValue(node[field], "supersession."+field, 160); err != nil {
			return err
		}
	}
	return nil
}

func validateStateSignature(node map[string]any, profileHash string,
	root map[string]any, label string) error {
	if node["trust_root_sha256"] != root["root_sha256"] ||
		node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("%s receipt does not bind lifecycle trust root", label)
	}
	key, err := lifecycleKey(root, stateKeyUsage)
	if err != nil {
		return err
	}
	_, err = validateSignature(node["signature"], label+".signature", profileHash,
		key["key_id"].(string))
	return err
}
