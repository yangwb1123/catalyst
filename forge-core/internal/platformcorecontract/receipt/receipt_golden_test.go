package receipt

import (
	"reflect"
	"testing"
)

func TestReceiptGoldenDigestsAndRoundTrips(t *testing.T) {
	fixture := loadReceiptGolden(t)
	if fixture.APIVersion != "forge.platform-core-receipt-fixture/v1" {
		t.Fatalf("fixture API = %q", fixture.APIVersion)
	}
	assertReceiptGolden(t, &fixture.ExecutionReceipt, fixture.Expected.ExecutionReceiptSHA256)
	assertVerificationRequestGolden(
		t, &fixture.VerificationRequest, fixture.Expected.VerificationRequestSHA256)
	assertVerificationReceiptGolden(
		t, &fixture.VerificationReceipt, fixture.Expected.VerificationReceiptSHA256)
	if err := ValidateVerificationExchange(
		&fixture.VerificationRequest, &fixture.VerificationReceipt); err != nil {
		t.Fatalf("golden exchange: %v", err)
	}
}

func assertReceiptGolden(t *testing.T, value *ExecutionReceipt, expected string) {
	t.Helper()
	canonical, err := CanonicalExecutionReceiptJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := ExecutionReceiptSHA256(value)
	if err != nil || digest != expected {
		t.Fatalf("execution digest = %q, %v", digest, err)
	}
	decoded, err := DecodeCanonicalExecutionReceipt(canonical)
	if err != nil || !reflect.DeepEqual(decoded, value) {
		t.Fatalf("execution round trip: %v", err)
	}
}

func assertVerificationRequestGolden(t *testing.T, value *VerificationRequest, expected string) {
	t.Helper()
	canonical, err := CanonicalVerificationRequestJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := VerificationRequestSHA256(value)
	if err != nil || digest != expected {
		t.Fatalf("request digest = %q, %v", digest, err)
	}
	decoded, err := DecodeCanonicalVerificationRequest(canonical)
	if err != nil || !reflect.DeepEqual(decoded, value) {
		t.Fatalf("request round trip: %v", err)
	}
}

func assertVerificationReceiptGolden(t *testing.T, value *VerificationReceipt, expected string) {
	t.Helper()
	canonical, err := CanonicalVerificationReceiptJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := VerificationReceiptSHA256(value)
	if err != nil || digest != expected {
		t.Fatalf("receipt digest = %q, %v", digest, err)
	}
	decoded, err := DecodeCanonicalVerificationReceipt(canonical)
	if err != nil || !reflect.DeepEqual(decoded, value) {
		t.Fatalf("verification receipt round trip: %v", err)
	}
}
