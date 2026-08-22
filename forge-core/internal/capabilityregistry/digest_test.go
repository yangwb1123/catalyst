package capabilityregistry

import "testing"

func TestCanonicalContentSetDigestMatchesIndependentPhysicalPreimage(t *testing.T) {
	raw := []byte(`{"files":[{"content_bytes":11167,"content_sha256":"7bbab5dd1fc2d630f8a1c3cdcae67aacdc0440c81d8e253f3578436c81e5e03c","media_type":"text/x-python","path":"harness/test_local_go_package_impact_prescan_bounds.py","selector":null},{"content_bytes":14826,"content_sha256":"ba637500ca1465bf275b3a0d29fbd94a9555c270674bc81f1f85ed2a5f04360a","media_type":"text/x-python","path":"harness/test_local_go_package_impact_prescan_contract_check.py","selector":null},{"content_bytes":1581,"content_sha256":"723f34d7069657d7fe2d63e3cc477edeadb4e76d4a4e1af5c128e148e0019b73","media_type":"text/x-python","path":"harness/local_go_package_impact_prescan_contract_check.py","selector":null}],"selection":{"mode":"explicit_files","root":null,"suffixes":[]},"set_sha256":""}`)
	value, err := parseStrictJSON(raw, maxContentSetBytes)
	if err != nil {
		t.Fatal(err)
	}
	document := value.(map[string]any)
	digest, err := digestDocument(contentSetDigestDomain, document, "set_sha256")
	if err != nil {
		t.Fatal(err)
	}
	const want = "3d7a072ffcaa6a222ae42ef6ac1b6135029ad5158fc38c70ce35f5afb3a28100"
	if digest != want {
		t.Fatalf("content-set digest = %s, want %s", digest, want)
	}
}
