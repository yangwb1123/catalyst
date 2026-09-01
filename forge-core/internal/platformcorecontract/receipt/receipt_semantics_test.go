package receipt

import (
	"bytes"
	"strings"
	"testing"

	"forgeos/forge-core/internal/platformcorecontract/state"
)

func TestExecutionReceiptRelationsFailClosed(t *testing.T) {
	mutations := []func(*ExecutionReceipt){
		func(value *ExecutionReceipt) { value.SessionRef.EntityID = "ses_0000000000000000000000000m" },
		func(value *ExecutionReceipt) { value.ObservedUsage.ElapsedMS-- },
		func(value *ExecutionReceipt) { value.TerminalState = state.AttemptState("running") },
		func(value *ExecutionReceipt) {
			value.OutputArtifactRefs[0].ProducerAttemptID = "atm_0000000000000000000000000m"
		},
		func(value *ExecutionReceipt) { value.EventRange.LastSequence = 8 },
	}
	for index, mutate := range mutations {
		value := loadReceiptGolden(t).ExecutionReceipt
		mutate(&value)
		if err := validateExecutionReceipt(&value); err == nil {
			t.Fatalf("mutation %d accepted", index)
		}
	}
}

func TestVerificationStatusIsStrictlyDerived(t *testing.T) {
	value := loadReceiptGolden(t).VerificationReceipt
	value.Results[1].Status = VerificationFail
	value.Results[1].ReasonCodes = []string{"test_failed"}
	value.OverallStatus = VerificationFail
	if err := validateVerificationReceipt(&value); err != nil {
		t.Fatalf("derived failure rejected: %v", err)
	}
	value.OverallStatus = VerificationPass
	if code := mustRejectionCode(t, validateVerificationReceipt(&value)); code != rejectionRelationMismatch {
		t.Fatalf("overall mismatch code = %s", code)
	}
}

func TestAllNotApplicableDerivesNotExecuted(t *testing.T) {
	value := loadReceiptGolden(t).VerificationReceipt
	value.Results = value.Results[:1]
	value.OverallStatus = VerificationNotExecuted
	if err := validateVerificationReceipt(&value); err != nil {
		t.Fatal(err)
	}
	value.OverallStatus = VerificationPass
	if err := validateVerificationReceipt(&value); err == nil {
		t.Fatal("all-N/A receipt was elevated to pass")
	}
}

func TestVerificationExchangeRequiresExactResultSet(t *testing.T) {
	fixture := loadReceiptGolden(t)
	fixture.VerificationReceipt.Results = fixture.VerificationReceipt.Results[:1]
	fixture.VerificationReceipt.OverallStatus = VerificationNotExecuted
	if err := validateVerificationReceipt(&fixture.VerificationReceipt); err != nil {
		t.Fatal(err)
	}
	err := ValidateVerificationExchange(
		&fixture.VerificationRequest, &fixture.VerificationReceipt)
	if code := mustRejectionCode(t, err); code != rejectionRelationMismatch {
		t.Fatalf("result-set mismatch code = %s", code)
	}
}

func TestVerificationExchangeUsesGlobalStageOrder(t *testing.T) {
	fixture := loadReceiptGolden(t)
	fixture.VerificationRequest.InputArtifactRef.ContentID = "sha256:" + strings.Repeat("0", 64)
	fixture.VerificationReceipt.OverallStatus = VerificationStatus("unknown")
	err := ValidateVerificationExchange(
		&fixture.VerificationRequest, &fixture.VerificationReceipt)
	if code := mustRejectionCode(t, err); code != rejectionStateInvalid {
		t.Fatalf("request relation masked receipt state: code = %s", code)
	}
}

func TestReceiptDecodersRejectFramingAndScalarDrift(t *testing.T) {
	fixture := loadReceiptGolden(t)
	canonical, err := CanonicalExecutionReceiptJSON(&fixture.ExecutionReceipt)
	if err != nil {
		t.Fatal(err)
	}
	cases := [][]byte{
		append([]byte(" "), canonical...),
		append(append([]byte{}, canonical...), '\n'),
		replaceReceiptBytes(t, canonical, `"execution_receipt_version":1`, `"execution_receipt_version":true`),
		replaceReceiptBytes(t, canonical, `{"approval_ref":`, `{"unexpected":true,"approval_ref":`),
	}
	for index, raw := range cases {
		if _, err := DecodeCanonicalExecutionReceipt(raw); err == nil {
			t.Fatalf("malformed case %d accepted", index)
		}
	}
}

func replaceReceiptBytes(t *testing.T, source []byte, old, replacement string) []byte {
	t.Helper()
	if !bytes.Contains(source, []byte(old)) {
		t.Fatalf("mutation source %q missing", old)
	}
	return bytes.Replace(source, []byte(old), []byte(replacement), 1)
}
