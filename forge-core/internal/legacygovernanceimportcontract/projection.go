package legacygovernanceimportcontract

import (
	"fmt"
	"sort"
)

func candidateBase(request map[string]any, source map[string]any,
	ordinal int64, raw []byte) (map[string]any, error) {
	locator := map[string]any{
		"ordinal": ordinal, "request_sha256": request["request_sha256"],
		"source_kind": source["source_kind"], "source_ref": source["source_ref"],
	}
	id, err := digestValue(candidateIDDomain, locator, maxRequestBytes, "candidate locator")
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"authority": nil, "candidate_id": id, "candidate_sha256": "",
		"current": false, "hardness": "none", "instruction_allowed": false,
		"ordinal": ordinal, "raw_byte_count": int64(len(raw)),
		"raw_bytes_base64url": encodeBase64URL(raw), "raw_sha256": shaBytes(raw),
		"request_sha256": request["request_sha256"], "source_kind": source["source_kind"],
		"source_ref": source["source_ref"], "trust_state": "unverified_legacy",
	}, nil
}

func sealCandidate(candidate map[string]any) error {
	digest, err := selfDigest(candidateDomain, candidate, "candidate_sha256",
		maxViewBytes, "candidate")
	if err != nil {
		return err
	}
	candidate["candidate_sha256"] = digest
	return nil
}

func memoryCandidates(request map[string]any, source map[string]any,
	raw []byte) ([]any, error) {
	entries, lines, err := parseMemoryJSONL(raw)
	if err != nil {
		return nil, err
	}
	result := make([]any, 0, len(entries))
	for index, entry := range entries {
		candidate, err := candidateBase(request, source, int64(index+1), lines[index])
		if err != nil {
			return nil, err
		}
		for field, value := range entry {
			candidate[field] = value
		}
		if err := sealCandidate(candidate); err != nil {
			return nil, err
		}
		result = append(result, candidate)
	}
	return result, nil
}

func adrCandidate(request map[string]any, source map[string]any,
	raw []byte) (map[string]any, error) {
	candidate, err := candidateBase(request, source, 1, raw)
	if err != nil {
		return nil, err
	}
	candidate["document_name"] = source["source_ref"]
	candidate["parsing"] = "not_performed"
	if err := sealCandidate(candidate); err != nil {
		return nil, err
	}
	return candidate, nil
}

type conflictKey struct {
	kind  string
	topic string
}

func conflictSets(requestSHA string, candidates []any) ([]any, error) {
	groups := make(map[conflictKey][]any)
	for _, value := range candidates {
		candidate := value.(map[string]any)
		if candidate["source_kind"] != memoryKind {
			continue
		}
		key := conflictKey{candidate["declared_kind"].(string),
			candidate["declared_topic"].(string)}
		groups[key] = append(groups[key], candidate["candidate_id"])
	}
	keys := make([]conflictKey, 0, len(groups))
	for key, members := range groups {
		if len(members) >= 2 {
			keys = append(keys, key)
		}
	}
	sort.Slice(keys, func(left, right int) bool {
		return keys[left].kind < keys[right].kind ||
			(keys[left].kind == keys[right].kind && keys[left].topic < keys[right].topic)
	})
	result := make([]any, 0, len(keys))
	for _, key := range keys {
		sort.Slice(groups[key], func(left, right int) bool {
			return groups[key][left].(string) < groups[key][right].(string)
		})
		for index := 1; index < len(groups[key]); index++ {
			if groups[key][index] == groups[key][index-1] {
				return nil, fmt.Errorf("conflict member candidate IDs must be unique")
			}
		}
		preimage := map[string]any{
			"declared_kind": key.kind, "declared_topic": key.topic,
			"member_candidate_ids": groups[key], "request_sha256": requestSHA,
		}
		id, err := digestValue(conflictDomain, preimage, maxViewBytes, "conflict set")
		if err != nil {
			return nil, err
		}
		result = append(result, map[string]any{
			"conflict_set_id": id, "declared_kind": key.kind,
			"declared_topic": key.topic, "member_candidate_ids": groups[key],
		})
	}
	return result, nil
}

func supersessions(requestSHA string, candidates []any) ([]any, error) {
	result := make([]any, 0)
	for _, value := range candidates {
		candidate := value.(map[string]any)
		target, ok := candidate["declared_supersedes"].(string)
		if !ok {
			continue
		}
		preimage := map[string]any{
			"declared_supersedes":    target,
			"declaring_candidate_id": candidate["candidate_id"],
			"request_sha256":         requestSHA,
		}
		id, err := digestValue(supersessionDomain, preimage, maxViewBytes,
			"supersession declaration")
		if err != nil {
			return nil, err
		}
		result = append(result, map[string]any{
			"declaration_id": id, "declared_supersedes": target,
			"declaring_candidate_id": candidate["candidate_id"],
			"relation_state":         "unresolved_unverified_legacy",
		})
	}
	return result, nil
}

// Project returns the unique canonical view document without stream framing.
func Project(requestWire []byte) ([]byte, error) {
	decoded, err := decodeRequest(requestWire)
	if err != nil {
		return nil, err
	}
	sources := decoded.value["sources"].([]any)
	candidates := make([]any, 0)
	for index, value := range sources {
		source := value.(map[string]any)
		if source["source_kind"] == memoryKind {
			items, err := memoryCandidates(decoded.value, source, decoded.rawSources[index])
			if err != nil {
				return nil, err
			}
			candidates = append(candidates, items...)
		} else {
			candidate, err := adrCandidate(decoded.value, source, decoded.rawSources[index])
			if err != nil {
				return nil, err
			}
			candidates = append(candidates, candidate)
		}
	}
	seenIDs := make(map[string]bool, len(candidates))
	for _, value := range candidates {
		id := value.(map[string]any)["candidate_id"].(string)
		if seenIDs[id] {
			return nil, fmt.Errorf("projected candidate IDs must be unique")
		}
		seenIDs[id] = true
	}
	return buildView(decoded.value, candidates)
}

func buildView(request map[string]any, candidates []any) ([]byte, error) {
	requestSHA := request["request_sha256"].(string)
	conflicts, err := conflictSets(requestSHA, candidates)
	if err != nil {
		return nil, err
	}
	declarations, err := supersessions(requestSHA, candidates)
	if err != nil {
		return nil, err
	}
	descriptors := sourceDescriptors(request)
	sourceSHA, err := digestValue(sourceSetDomain, descriptors, maxViewBytes,
		"source descriptor set")
	if err != nil {
		return nil, err
	}
	view := map[string]any{
		"api_version": viewAPI,
		"attestations": map[string]any{
			"acceptance": false, "authority": false, "confidence_interpretation": false,
			"conflict_resolution": false, "currentness": false,
			"instruction_eligibility": false, "persistence": false, "runtime_effect": false,
			"source_authentication": false, "source_completeness": false,
			"status_interpretation": false, "truth": false, "winner_selection": false,
		},
		"binding": request["binding"], "candidates": candidates,
		"canonicalization": canonicalization, "conflict_sets": conflicts,
		"declared_supersessions": declarations, "kind": viewKind,
		"request_sha256": requestSHA, "result": result,
		"source_set_sha256": sourceSHA, "sources": descriptors, "view_sha256": "",
	}
	digest, err := selfDigest(viewDomain, view, "view_sha256", maxViewBytes, "view")
	if err != nil {
		return nil, err
	}
	view["view_sha256"] = digest
	encoded, err := canonicalJSON(view, maxViewBytes, "view")
	if err != nil {
		return nil, fmt.Errorf("encode view: %w", err)
	}
	return encoded, nil
}
