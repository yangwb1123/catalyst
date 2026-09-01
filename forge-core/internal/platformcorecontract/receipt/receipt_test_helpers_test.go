package receipt

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type receiptGoldenFixture struct {
	APIVersion          string               `json:"api_version"`
	ExecutionReceipt    ExecutionReceipt     `json:"execution_receipt"`
	Expected            receiptGoldenDigests `json:"expected"`
	VerificationReceipt VerificationReceipt  `json:"verification_receipt"`
	VerificationRequest VerificationRequest  `json:"verification_request"`
}

type receiptGoldenDigests struct {
	ExecutionReceiptSHA256    string `json:"execution_receipt_sha256"`
	VerificationReceiptSHA256 string `json:"verification_receipt_sha256"`
	VerificationRequestSHA256 string `json:"verification_request_sha256"`
}

func loadReceiptGolden(t *testing.T) receiptGoldenFixture {
	t.Helper()
	raw, err := os.ReadFile("../../../../docs/contracts/fixtures/platform-core-receipt-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var fixture receiptGoldenFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatal(err)
	}
	return fixture
}

func mustRejectionCode(t *testing.T, err error) core.RejectionCode {
	t.Helper()
	if err == nil {
		t.Fatal("expected coded rejection")
	}
	code, ok := core.RejectionCodeOf(err)
	if !ok {
		t.Fatalf("uncoded rejection: %v", err)
	}
	return code
}
