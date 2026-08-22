package contextpackagecontract

import (
	"strings"
	"testing"
)

func singleSourceRequest(t *testing.T, index int) *BuildRequest {
	t.Helper()
	request := validRequest(t)
	source := request.Sources[index]
	request.Sources = []Source{source}
	request.Redactions = []Redaction{}
	return request
}

func TestOptionalIneligibleReasonsAreExplicit(t *testing.T) {
	cases := map[string]func(*Source){
		"missing": func(source *Source) {
			source.Availability, source.Content, source.ContentSHA256 = "missing", nil, nil
		},
		"denied":            func(source *Source) { source.Disposition = "deny" },
		"stale":             func(source *Source) { source.Freshness = "stale" },
		"contested":         func(source *Source) { source.Freshness = "contested" },
		"unknown_freshness": func(source *Source) { source.Freshness = "unknown" },
		"expired": func(source *Source) {
			value := int64(1700000000000)
			source.ExpiresAtUnixMS = &value
		},
		"quarantined_prompt_injection": func(source *Source) { source.InjectionRisk = "suspected" },
	}
	for reason, mutate := range cases {
		t.Run(reason, func(t *testing.T) {
			request := singleSourceRequest(t, 1)
			mutate(&request.Sources[0])
			packageValue, err := Assemble(request, byteCounter{})
			if err != nil {
				t.Fatal(err)
			}
			if len(packageValue.Omissions) != 1 || packageValue.Omissions[0].Reason != reason {
				t.Fatalf("omissions = %#v", packageValue.Omissions)
			}
		})
	}
}

func TestRequiredIneligibleSourcesFailClosed(t *testing.T) {
	for reason, mutate := range map[string]func(*Source){
		"missing": func(source *Source) {
			source.Availability, source.Content, source.ContentSHA256 = "missing", nil, nil
		},
		"denied":    func(source *Source) { source.Disposition = "deny" },
		"stale":     func(source *Source) { source.Freshness = "stale" },
		"contested": func(source *Source) { source.Freshness = "contested" },
		"unknown":   func(source *Source) { source.Freshness = "unknown" },
		"expired": func(source *Source) {
			value := int64(1700000000000)
			source.ExpiresAtUnixMS = &value
		},
		"injection": func(source *Source) { source.InjectionRisk = "suspected" },
	} {
		t.Run(reason, func(t *testing.T) {
			request := singleSourceRequest(t, 1)
			request.Sources[0].Required = true
			mutate(&request.Sources[0])
			if _, err := Assemble(request, byteCounter{}); err == nil {
				t.Fatal("expected required source failure")
			}
		})
	}
}

func TestUTF8TruncationAndEmptyPrefix(t *testing.T) {
	request := singleSourceRequest(t, 2)
	content := "αsuffix"
	digest := contentDigest(content)
	request.Sources[0].Content, request.Sources[0].ContentSHA256 = &content, &digest
	request.Sources[0].MaxBytes, request.Sources[0].Truncation = 2, "utf8_prefix"
	packageValue, err := Assemble(request, byteCounter{})
	if err != nil {
		t.Fatal(err)
	}
	snippet := packageValue.Lanes.UntrustedData[0]
	if snippet.Content != "α" || snippet.Truncation == nil || snippet.Truncation.RetainedBytes != 2 {
		t.Fatalf("unexpected truncation: %#v", snippet)
	}
	request.Sources[0].MaxBytes = 1
	packageValue, err = Assemble(request, byteCounter{})
	if err != nil {
		t.Fatal(err)
	}
	if packageValue.Omissions[0].Reason != "source_limit_exceeded" || packageValue.Accounting.SelectedSnippetCount != 0 {
		t.Fatalf("empty prefix was not omitted: %#v", packageValue)
	}
}

func TestRequiredSourceLimitAndAggregateBudgetsFail(t *testing.T) {
	t.Run("source max", func(t *testing.T) {
		request := singleSourceRequest(t, 0)
		request.Sources[0].MaxBytes = 1
		request.Sources[0].Truncation = "utf8_prefix"
		if _, err := Assemble(request, byteCounter{}); err == nil {
			t.Fatal("expected required max_bytes failure")
		}
	})
	t.Run("content", func(t *testing.T) {
		request := singleSourceRequest(t, 0)
		request.Budget.MaxContentBytes = 1
		if _, err := Assemble(request, byteCounter{}); err == nil {
			t.Fatal("expected required content budget failure")
		}
	})
	t.Run("token", func(t *testing.T) {
		request := singleSourceRequest(t, 0)
		baseline, err := countProjection(byteCounter{}, request.Budget, emptyLanes())
		if err != nil {
			t.Fatal(err)
		}
		request.Budget.MaxTokens = baseline
		if _, err := Assemble(request, byteCounter{}); err == nil {
			t.Fatal("expected required token budget failure")
		}
	})
}

func TestOptionalBudgetOmissionPrecedence(t *testing.T) {
	t.Run("content", func(t *testing.T) {
		request := singleSourceRequest(t, 1)
		request.Budget.MaxContentBytes = 1
		packageValue, err := Assemble(request, byteCounter{})
		if err != nil || packageValue.Omissions[0].Reason != "content_budget_exceeded" {
			t.Fatalf("package=%#v err=%v", packageValue, err)
		}
	})
	t.Run("token", func(t *testing.T) {
		request := singleSourceRequest(t, 1)
		baseline, err := countProjection(byteCounter{}, request.Budget, emptyLanes())
		if err != nil {
			t.Fatal(err)
		}
		request.Budget.MaxTokens = baseline
		packageValue, err := Assemble(request, byteCounter{})
		if err != nil || packageValue.Omissions[0].Reason != "token_budget_exceeded" {
			t.Fatalf("package=%#v err=%v", packageValue, err)
		}
	})
	t.Run("snippet before content", func(t *testing.T) {
		request := validRequest(t)
		packageValue, err := Assemble(request, byteCounter{})
		if err != nil {
			t.Fatal(err)
		}
		if packageValue.Omissions[1].Reason != "snippet_budget_exceeded" {
			t.Fatalf("unexpected omission %v", packageValue.Omissions[1])
		}
	})
}

func TestRedactionReceiptsDoNotExposePreimage(t *testing.T) {
	request := singleSourceRequest(t, 0)
	request.Redactions = validRequest(t).Redactions
	request.Sources[0].Required, request.Sources[0].Disposition = false, "deny"
	packageValue, err := Assemble(request, byteCounter{})
	if err != nil {
		t.Fatal(err)
	}
	if len(packageValue.RedactionReceipts) != 1 || packageValue.Accounting.RedactedRangeCount != 1 {
		t.Fatalf("declared receipt missing: %#v", packageValue.RedactionReceipts)
	}
	encoded, err := CanonicalPackageJSON(packageValue)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(encoded), "SECRET") {
		t.Fatal("redaction preimage leaked into package")
	}
}
