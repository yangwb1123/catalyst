package planningownership

import (
	"bytes"
	"encoding/base64"
	"fmt"
)

// BuildRequest seals two explicit exact YAML byte strings into a canonical request.
func BuildRequest(catalogBytes, mappingBytes []byte) (Request, error) {
	if len(catalogBytes) == 0 || len(catalogBytes) > maxCatalogSourceBytes ||
		len(mappingBytes) == 0 || len(mappingBytes) > maxMappingSourceBytes {
		return Request{}, fmt.Errorf("source byte length rejected before parsing")
	}
	catalogBytes, mappingBytes = cloneBytes(catalogBytes), cloneBytes(mappingBytes)
	catalog, err := decodeCatalog(catalogBytes)
	if err != nil {
		return Request{}, fmt.Errorf("catalog source rejected: %w", err)
	}
	mapping, err := decodeMapping(mappingBytes)
	if err != nil {
		return Request{}, fmt.Errorf("mapping source rejected: %w", err)
	}
	if err := requireCompleteCoverage(catalog, mapping); err != nil {
		return Request{}, fmt.Errorf("source coverage rejected: %w", err)
	}
	document := buildRequestDocument(catalogBytes, mappingBytes)
	digest, err := documentDigest(requestDigestDomain, document, "request_sha256")
	if err != nil {
		return Request{}, err
	}
	document["request_sha256"] = digest
	encoded, err := canonicalJSON(document)
	if err != nil || len(encoded) > maxRequestBytes {
		return Request{}, fmt.Errorf("request encoding rejected")
	}
	return Request{document: document, encoded: encoded, catalog: catalogBytes, mapping: mappingBytes}, nil
}

func buildRequestDocument(catalogBytes, mappingBytes []byte) map[string]any {
	return map[string]any{
		"api_version": requestAPIVersion, "canonicalization": canonicalization,
		"catalog_source": sourceRecord(catalogBytes, catalogDocumentName, "capability_catalog"),
		"kind":           requestKind,
		"mapping_source": sourceRecord(mappingBytes, mappingDocumentName, "capability_skill_map"),
		"request_sha256": "",
	}
}

func sourceRecord(raw []byte, documentName, role string) map[string]any {
	return map[string]any{
		"content_base64": base64.StdEncoding.EncodeToString(raw), "content_bytes": int64(len(raw)),
		"content_encoding": "base64-rfc4648-canonical", "content_sha256": rawSHA256(raw),
		"document_name": documentName, "media_type": "application/yaml", "source_role": role,
	}
}

// DecodeRequest validates exact canonical request bytes and both embedded sources.
func DecodeRequest(data []byte) (Request, error) {
	document, err := parseCanonicalObject(data, maxRequestBytes)
	if err != nil {
		return Request{}, err
	}
	catalog, mapping, err := validateRequestDocument(document)
	if err != nil {
		return Request{}, err
	}
	return Request{document: document, encoded: cloneBytes(data), catalog: catalog, mapping: mapping}, nil
}

func validateRequestDocument(document map[string]any) ([]byte, []byte, error) {
	keys := []string{"api_version", "canonicalization", "catalog_source", "kind", "mapping_source", "request_sha256"}
	if err := requireKeys(document, keys); err != nil {
		return nil, nil, fmt.Errorf("projection request: %w", err)
	}
	for field, expected := range map[string]string{
		"api_version": requestAPIVersion, "canonicalization": canonicalization, "kind": requestKind,
	} {
		if err := requireString(document, field, expected); err != nil {
			return nil, nil, err
		}
	}
	catalogRecord, err := objectValue(document, "catalog_source")
	if err != nil {
		return nil, nil, err
	}
	mappingRecord, err := objectValue(document, "mapping_source")
	if err != nil {
		return nil, nil, err
	}
	catalog, err := validateSourceRecord(catalogRecord, catalogDocumentName, "capability_catalog", maxCatalogSourceBytes)
	if err != nil {
		return nil, nil, fmt.Errorf("catalog source: %w", err)
	}
	mapping, err := validateSourceRecord(mappingRecord, mappingDocumentName, "capability_skill_map", maxMappingSourceBytes)
	if err != nil {
		return nil, nil, fmt.Errorf("mapping source: %w", err)
	}
	if err := requireDigest(requestDigestDomain, document, "request_sha256"); err != nil {
		return nil, nil, err
	}
	catalogView, err := decodeCatalog(catalog)
	if err != nil {
		return nil, nil, err
	}
	mappingView, err := decodeMapping(mapping)
	if err != nil {
		return nil, nil, err
	}
	if err := requireCompleteCoverage(catalogView, mappingView); err != nil {
		return nil, nil, err
	}
	return catalog, mapping, nil
}

func validateSourceRecord(record map[string]any, name, role string, maximum int) ([]byte, error) {
	keys := []string{"content_base64", "content_bytes", "content_encoding", "content_sha256", "document_name", "media_type", "source_role"}
	if err := requireKeys(record, keys); err != nil {
		return nil, err
	}
	constants := map[string]string{
		"content_encoding": "base64-rfc4648-canonical", "document_name": name,
		"media_type": "application/yaml", "source_role": role,
	}
	for key, expected := range constants {
		if err := requireString(record, key, expected); err != nil {
			return nil, err
		}
	}
	encoded, err := stringValue(record, "content_base64", 4, base64.StdEncoding.EncodedLen(maximum))
	if err != nil {
		return nil, err
	}
	raw, err := base64.StdEncoding.Strict().DecodeString(encoded)
	if err != nil || base64.StdEncoding.EncodeToString(raw) != encoded || len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("source content_base64 is not canonical or bounded")
	}
	count, err := integerValue(record, "content_bytes", 1, int64(maximum))
	digest, digestErr := stringValue(record, "content_sha256", 64, 64)
	if err != nil || digestErr != nil || int64(len(raw)) != count || !validHash(digest) || rawSHA256(raw) != digest {
		return nil, fmt.Errorf("source byte count or digest mismatch")
	}
	return raw, nil
}

func requestEqual(left, right Request) bool {
	return bytes.Equal(left.encoded, right.encoded)
}
