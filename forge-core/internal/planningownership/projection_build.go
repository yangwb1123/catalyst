package planningownership

import (
	"fmt"
	"sort"
)

// Project deterministically derives complete planning ownership from a sealed request.
func Project(request Request) (Projection, error) {
	if err := request.valid(); err != nil {
		return Projection{}, err
	}
	redecoded, err := DecodeRequest(request.encoded)
	if err != nil || !requestEqual(request, redecoded) {
		return Projection{}, fmt.Errorf("request integrity rejected")
	}
	catalog, err := decodeCatalog(redecoded.catalog)
	if err != nil {
		return Projection{}, err
	}
	mapping, err := decodeMapping(redecoded.mapping)
	if err != nil {
		return Projection{}, err
	}
	if err := requireCompleteCoverage(catalog, mapping); err != nil {
		return Projection{}, err
	}
	requestDigest := redecoded.document["request_sha256"].(string)
	bindings, occurrenceCount, err := buildBindings(catalog, mapping, requestDigest)
	if err != nil {
		return Projection{}, err
	}
	document := buildProjectionDocument(redecoded.document, bindings, catalog, mapping, occurrenceCount)
	digest, err := documentDigest(projectionDigestDomain, document, "projection_sha256")
	if err != nil {
		return Projection{}, err
	}
	document["projection_sha256"] = digest
	encoded, err := canonicalJSON(document)
	if err != nil || len(encoded) > maxProjectionBytes {
		return Projection{}, fmt.Errorf("projection encoding rejected")
	}
	return Projection{document: document, encoded: encoded}, nil
}

func requireCompleteCoverage(catalog catalogView, mapping mappingView) error {
	if len(catalog.occurrences) != len(mapping.owners) {
		return fmt.Errorf("catalog and ownership capability counts differ")
	}
	for capability := range catalog.occurrences {
		if _, exists := mapping.owners[capability]; !exists {
			return fmt.Errorf("catalog capability %q has no primary owner", capability)
		}
	}
	for capability := range mapping.owners {
		if _, exists := catalog.occurrences[capability]; !exists {
			return fmt.Errorf("ownership capability %q is absent from catalog", capability)
		}
	}
	return nil
}

func buildBindings(catalog catalogView, mapping mappingView, requestDigest string) ([]any, int64, error) {
	capabilities := sortedCapabilities(catalog.occurrences)
	bindings := make([]any, 0, len(capabilities))
	var total int64
	for _, capability := range capabilities {
		nodes := append([]string(nil), catalog.occurrences[capability]...)
		sort.Strings(nodes)
		owner := mapping.owners[capability]
		binding := buildBinding(capability, nodes, owner, requestDigest)
		digest, err := documentDigest(bindingDigestDomain, binding, "binding_sha256")
		if err != nil {
			return nil, 0, err
		}
		binding["binding_sha256"] = digest
		encoded, err := canonicalJSON(binding)
		if err != nil || len(encoded) > maxBindingBytes {
			return nil, 0, fmt.Errorf("binding %q is oversized", capability)
		}
		bindings = append(bindings, binding)
		total += int64(len(nodes))
	}
	return bindings, total, nil
}

func buildBinding(capability string, nodes []string, owner ownerRecord, requestDigest string) map[string]any {
	nodeValues := make([]any, len(nodes))
	for index, node := range nodes {
		nodeValues[index] = node
	}
	return map[string]any{
		"binding_sha256": "", "capability_id": capability, "catalog_node_ids": nodeValues,
		"catalog_occurrence_count":     int64(len(nodes)),
		"declared_logical_adapter_ref": ".agent/skills/" + owner.skill + ".md",
		"implementation_wave":          owner.wave, "owner_skill": owner.skill,
		"physical_resolution": "not_performed", "request_sha256": requestDigest,
		"skill_availability": "not_evaluated",
	}
}

func buildProjectionDocument(
	request map[string]any, bindings []any, catalog catalogView, mapping mappingView, occurrences int64,
) map[string]any {
	requestDigest := request["request_sha256"].(string)
	return map[string]any{
		"api_version": projectionAPIVersion, "authority_semantics": authoritySemantics(),
		"bindings": bindings, "canonicalization": canonicalization,
		"coverage": map[string]any{
			"binding_count": int64(len(bindings)), "capability_occurrence_count": occurrences,
			"catalog_node_count": int64(catalog.nodeCount), "mapped_capability_count": int64(len(mapping.owners)),
			"mapping_package_count": int64(mapping.packageCount), "unique_capability_count": int64(len(catalog.occurrences)),
			"unmapped_capability_ids": []any{}, "unreferenced_mapping_capability_ids": []any{},
		},
		"kind": projectionKind, "positive_result": positiveResult, "projection_mode": projectionMode,
		"projection_sha256": "", "request": cloneObject(request), "request_sha256": requestDigest,
		"status": "planning_only",
	}
}

func authoritySemantics() map[string]any {
	return map[string]any{
		"adapter_file_existence": "not_evaluated", "adapter_skill_availability": "not_evaluated",
		"attestations": []any{}, "authentication_attestation": false, "authorization_decision": "none",
		"capability_invocation": false, "capability_registry_mutation": false,
		"effect_attestation": false, "execution_attestation": false, "grant_or_pdp_activation": false,
		"implementation_selection": false, "ownership_authority_attestation": false,
		"permission_attestation": false, "persistence": "none", "runtime_routing": false,
		"source_authentication": false, "transition_attestation": false,
	}
}
