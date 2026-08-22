package contextpackagecontract

import "testing"

func TestRequestRequiresSortedUniqueSourcesAndRefs(t *testing.T) {
	t.Run("source order", func(t *testing.T) {
		request := validRequest(t)
		request.Sources[0], request.Sources[1] = request.Sources[1], request.Sources[0]
		if _, err := CanonicalRequestJSON(request); err == nil {
			t.Fatal("expected source order rejection")
		}
	})
	t.Run("duplicate ref", func(t *testing.T) {
		request := validRequest(t)
		request.Sources[1].SourceRef = request.Sources[0].SourceRef
		if _, err := CanonicalRequestJSON(request); err == nil {
			t.Fatal("expected duplicate source_ref rejection")
		}
	})
}

func TestRequestEnforcesAvailabilityAndPlainContentDigest(t *testing.T) {
	t.Run("missing retains content", func(t *testing.T) {
		request := validRequest(t)
		request.Redactions = []Redaction{}
		request.Sources[0].Availability = "missing"
		if _, err := CanonicalRequestJSON(request); err == nil {
			t.Fatal("expected missing/content mismatch")
		}
	})
	t.Run("available empty", func(t *testing.T) {
		request := validRequest(t)
		request.Redactions = []Redaction{}
		request.Sources[0].Content = stringPointer("")
		digest := contentDigest("")
		request.Sources[0].ContentSHA256 = &digest
		if _, err := CanonicalRequestJSON(request); err == nil {
			t.Fatal("expected empty content rejection")
		}
	})
	t.Run("wrong digest", func(t *testing.T) {
		request := validRequest(t)
		request.Sources[0].ContentSHA256 = stringPointer("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
		if _, err := CanonicalRequestJSON(request); err == nil {
			t.Fatal("expected content digest rejection")
		}
	})
}

func TestUntrustedSourceCannotEscalateLaneOrTrust(t *testing.T) {
	for name, mutate := range map[string]func(*Source){
		"instruction lane": func(source *Source) { source.DeclaredLane = "instruction" },
		"trusted lane":     func(source *Source) { source.DeclaredLane = "trusted_context" },
		"trusted claim":    func(source *Source) { source.DeclaredTrust = "project_governance" },
	} {
		t.Run(name, func(t *testing.T) {
			request := validRequest(t)
			mutate(&request.Sources[2])
			if _, err := CanonicalRequestJSON(request); err == nil {
				t.Fatal("expected trust-boundary rejection")
			}
		})
	}
}

func TestRedactionRangesRequireOrderedUTF8Boundaries(t *testing.T) {
	request := validRequest(t)
	request.Redactions = []Redaction{{
		SourceID: "source-03-repository",
		Ranges:   []RedactionRange{{StartByte: 18, EndByte: 19, RuleID: "inside-alpha"}},
	}}
	if _, err := CanonicalRequestJSON(request); err == nil {
		t.Fatal("expected split UTF-8 range rejection")
	}
	request = validRequest(t)
	request.Redactions[0].Ranges = []RedactionRange{
		{StartByte: 13, EndByte: 19, RuleID: "first"},
		{StartByte: 18, EndByte: 20, RuleID: "overlap"},
	}
	if _, err := CanonicalRequestJSON(request); err == nil {
		t.Fatal("expected overlapping range rejection")
	}
}
