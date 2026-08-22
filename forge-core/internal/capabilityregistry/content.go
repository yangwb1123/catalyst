package capabilityregistry

import (
	"fmt"
	"strings"
)

func validateContentRef(value map[string]any, schemaOnly bool) error {
	if err := requireKeys(value, "content_bytes", "content_sha256", "media_type", "path", "selector"); err != nil {
		return err
	}
	if _, err := integerValue(value, "content_bytes", 0, 16<<20); err != nil {
		return err
	}
	digest, err := stringValue(value, "content_sha256", 64, 64)
	if err != nil || !validHash(digest) {
		return fmt.Errorf("content_sha256 must be lowercase SHA-256")
	}
	mediaType, err := stringValue(value, "media_type", 1, 64)
	if err != nil || !oneOf(mediaType, "application/json", "application/schema+json",
		"text/markdown", "text/x-go", "text/x-python") {
		return fmt.Errorf("content ref media_type is unsupported")
	}
	if schemaOnly && mediaType != "application/schema+json" {
		return fmt.Errorf("schema ref media_type must be application/schema+json")
	}
	path, err := stringValue(value, "path", 1, maxRepoPathBytes)
	if err != nil || !validRepoPath(path) {
		return fmt.Errorf("content ref path is invalid")
	}
	return validateSelector(value["selector"])
}

func validateSelector(value any) error {
	if value == nil {
		return nil
	}
	selector, ok := value.(string)
	if !ok || len(selector) > maxRepoPathBytes || validateWireString(selector) != nil ||
		!validJSONPointer(selector, true) {
		return fmt.Errorf("content ref selector is invalid")
	}
	return nil
}

func validateContentSet(value map[string]any) error {
	if err := requireKeys(value, "files", "selection", "set_sha256"); err != nil {
		return err
	}
	files, err := arrayValue(value, "files", 1, maxContentFiles)
	if err != nil {
		return err
	}
	if err := validateContentRefs(files, false); err != nil {
		return fmt.Errorf("content set files: %w", err)
	}
	selection, err := objectValue(value, "selection")
	if err != nil {
		return err
	}
	if err := validateContentSelection(selection); err != nil {
		return err
	}
	digest, err := stringValue(value, "set_sha256", 64, 64)
	if err != nil || !validHash(digest) {
		return fmt.Errorf("content set digest is invalid")
	}
	if err := requireDigest(value, contentSetDigestDomain, "set_sha256"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxContentSetBytes, "content set")
}

func validateContentRefs(values []any, schemaOnly bool) error {
	objects, err := requireObjectItems(values)
	if err != nil {
		return err
	}
	previous := ""
	locators := make(map[string]struct{}, len(objects))
	for index, object := range objects {
		if err := validateContentRef(object, schemaOnly); err != nil {
			return fmt.Errorf("item %d: %w", index, err)
		}
		encoded, _ := canonicalJSON(object)
		identity := string(encoded)
		if index > 0 && identity <= previous {
			return fmt.Errorf("content refs must be canonical-byte sorted and unique")
		}
		locator := contentRefLocator(object)
		if _, exists := locators[locator]; exists {
			return fmt.Errorf("content refs duplicate a path/selector tuple")
		}
		locators[locator] = struct{}{}
		previous = identity
	}
	return nil
}

func contentRefLocator(value map[string]any) string {
	selector := "<null>"
	if value["selector"] != nil {
		selector = value["selector"].(string)
	}
	return value["path"].(string) + "\x00" + selector
}

func validateContentSelection(value map[string]any) error {
	if err := requireKeys(value, "mode", "root", "suffixes"); err != nil {
		return err
	}
	mode, err := stringValue(value, "mode", 1, 64)
	if err != nil || !oneOf(mode, "all_regular_files_recursive_with_suffixes", "explicit_files") {
		return fmt.Errorf("content-set selection mode is invalid")
	}
	suffixes, err := arrayValue(value, "suffixes", 0, 16)
	if err != nil || requireSortedUniqueStrings(suffixes, validSuffix) != nil {
		return fmt.Errorf("content-set suffixes must be sorted unique suffixes")
	}
	if mode == "explicit_files" {
		if value["root"] != nil || len(suffixes) != 0 {
			return fmt.Errorf("explicit selection requires null root and empty suffixes")
		}
		return nil
	}
	root, ok := value["root"].(string)
	if !ok || !validRepoPath(root) || len(suffixes) == 0 {
		return fmt.Errorf("recursive selection requires a valid root and suffixes")
	}
	return nil
}

func validSuffix(value string) bool {
	if len(value) < 2 || len(value) > 32 || value[0] != '.' {
		return false
	}
	for _, character := range []byte(value[1:]) {
		if (character < 'a' || character > 'z') && (character < '0' || character > '9') {
			return false
		}
	}
	return true
}

func forbiddenRegistryRef(path string) bool {
	return path == "docs/contracts/capability-registry-v1.schema.json" ||
		path == "docs/contracts/fixtures/capability-registry-v1.json" ||
		strings.Contains(path, "ADR-0068-authority-neutral-capability-registry-v1") ||
		strings.Contains(path, "capability_registry") || strings.Contains(path, "capability-registry")
}

func requireCanonicalSize(value any, maximum int, name string) error {
	encoded, err := canonicalJSON(value)
	if err != nil {
		return err
	}
	if len(encoded) > maximum {
		return fmt.Errorf("%s exceeds %d bytes", name, maximum)
	}
	return nil
}
