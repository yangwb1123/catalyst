package receipt

import (
	"fmt"
	"strings"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/state"
)

func TestVerificationCheckCountBoundary(t *testing.T) {
	request := loadReceiptGolden(t).VerificationRequest
	request.Checks = make([]VerificationCheckRequest, maxVerificationChecks)
	for index := range request.Checks {
		request.Checks[index] = VerificationCheckRequest{
			CheckID: fmt.Sprintf("check_%02d", index), CheckName: "forge.check.test",
			DeclaredRequired: true,
		}
	}
	if err := validateVerificationRequest(&request); err != nil {
		t.Fatalf("N checks rejected: %v", err)
	}
	request.Checks = append(request.Checks, VerificationCheckRequest{
		CheckID: "check_extra", CheckName: "forge.check.test", DeclaredRequired: true,
	})
	if err := validateVerificationRequest(&request); err == nil {
		t.Fatal("N+1 checks accepted")
	}
}

func TestExecutionArtifactCountBoundary(t *testing.T) {
	receipt := loadReceiptGolden(t).ExecutionReceipt
	source := receipt.OutputArtifactRefs[0]
	receipt.OutputArtifactRefs = make([]core.ArtifactRef, maxReceiptArtifacts)
	for index := range receipt.OutputArtifactRefs {
		receipt.OutputArtifactRefs[index] = numberedArtifact(source, index)
	}
	if err := validateExecutionReceipt(&receipt); err != nil {
		t.Fatalf("N artifacts rejected: %v", err)
	}
	receipt.OutputArtifactRefs = append(
		receipt.OutputArtifactRefs, numberedArtifact(source, maxReceiptArtifacts))
	if err := validateExecutionReceipt(&receipt); err == nil {
		t.Fatal("N+1 artifacts accepted")
	}
}

func TestExecutionReasonCountBoundary(t *testing.T) {
	receipt := loadReceiptGolden(t).ExecutionReceipt
	receipt.TerminalState = state.AttemptState("failed")
	receipt.ReasonCodes = make([]string, maxReasonCodes)
	for index := range receipt.ReasonCodes {
		receipt.ReasonCodes[index] = fmt.Sprintf("reason_%02d", index)
	}
	if err := validateExecutionReceipt(&receipt); err != nil {
		t.Fatalf("N reasons rejected: %v", err)
	}
	receipt.ReasonCodes = append(receipt.ReasonCodes, "reason_extra")
	if err := validateExecutionReceipt(&receipt); err == nil {
		t.Fatal("N+1 reasons accepted")
	}
}

func TestVerificationReceiptWholeDocumentBoundPrecedesState(t *testing.T) {
	receipt := oversizedVerificationReceipt(t)
	receipt.OverallStatus = VerificationStatus("unknown")
	if code := mustRejectionCode(t, validateVerificationReceipt(&receipt)); code != rejectionValueInvalid {
		t.Fatalf("whole-document bound/state code = %s", code)
	}
}

func TestVerificationExchangeBoundsBothDocumentsBeforeValues(t *testing.T) {
	request := loadReceiptGolden(t).VerificationRequest
	request.VerificationID = "atm_0000000000000000000000000g"
	receipt := oversizedVerificationReceipt(t)
	if code := mustRejectionCode(t, ValidateVerificationExchange(&request, &receipt)); code != rejectionValueInvalid {
		t.Fatalf("receipt bound/request identifier code = %s", code)
	}
}

func oversizedVerificationReceipt(t *testing.T) VerificationReceipt {
	receipt := loadReceiptGolden(t).VerificationReceipt
	receipt.Results = make([]VerificationCheckResult, maxVerificationChecks)
	evidence := maximalEvidenceRefs()
	for index := range receipt.Results {
		receipt.Results[index] = VerificationCheckResult{
			Applicability: VerificationApplicable,
			CheckID:       fmt.Sprintf("check_%02d", index),
			EvidenceRefs:  evidence,
			ReasonCodes:   []string{},
			Status:        VerificationPass,
		}
	}
	return receipt
}

func maximalEvidenceRefs() []core.RecordRef {
	recordType := strings.Repeat("a", 31) + "." + strings.Repeat("b", 31) + "." +
		strings.Repeat("c", 31) + "." + strings.Repeat("d", 32)
	values := make([]core.RecordRef, maxEvidenceRefs)
	for index := range values {
		prefix := fmt.Sprintf("evidence_%04d_", index)
		values[index] = core.RecordRef{
			RecordID:     prefix + strings.Repeat("a", 160-len(prefix)),
			RecordSHA256: strings.Repeat("a", 64), RecordType: recordType,
		}
	}
	return values
}

func numberedArtifact(source core.ArtifactRef, number int) core.ArtifactRef {
	alphabet := "0123456789abcdefghjkmnpqrstvwxyz"
	high, low := number/len(alphabet), number%len(alphabet)
	source.LogicalID = "art_" + "000000000000000000000000" +
		string(alphabet[high]) + string(alphabet[low])
	return source
}
