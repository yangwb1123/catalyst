package receipt

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"

	core "forgeos/forge-core/internal/platformcorecontract"
	contractwire "forgeos/forge-core/internal/platformcorecontract/internal/wireprofile"
)

func typedCanonical(value any, maximum int) ([]byte, error) {
	return contractwire.TypedCanonical(value, maximum)
}

func decodeTypedCanonical(data []byte, maximum int, target any) error {
	return contractwire.DecodeTypedCanonical(data, maximum, target)
}

func validateExactTypedDocument(data []byte, value any, label string) error {
	canonical, err := typedCanonical(value, maxReceiptBytes)
	if err != nil {
		return withRejection(err, rejectionDocumentInvalid)
	}
	if !bytes.Equal(data, canonical) {
		return rejectf(rejectionDocumentInvalid, "%s is missing required exact fields", label)
	}
	return nil
}

func canonicalJSON(value any, maximum int) ([]byte, error) {
	return contractwire.CanonicalJSON(value, maximum)
}

func parseStrictJSON(data []byte, maximum int) (any, error) {
	return contractwire.ParseStrictJSON(data, maximum)
}

func validateTypedID(value, prefix, label string) error {
	if err := core.ValidatePlatformID(value); err != nil {
		return err
	}
	if !strings.HasPrefix(value, prefix+"_") {
		return rejectf(rejectionIdentifierInvalid, "%s must use %s_", label, prefix)
	}
	return nil
}

func validateEntityRef(value core.EntityRef, _ string) error {
	return core.ValidateReference(value)
}

func validateActorRef(value core.ActorRef) error {
	return core.ValidateReference(value)
}

func validateRecordRef(value core.RecordRef, _ string) error {
	return core.ValidateReference(value)
}

func validateScopeValues(value core.ScopeRef) error {
	return core.ValidateReference(value, "values")
}

func validateScopeReferences(value core.ScopeRef) error {
	return core.ValidateReference(value, "references")
}

func validateArtifactValues(value *core.ArtifactRef) error {
	return core.ValidateArtifactRef(value, "values")
}

func validateArtifactReferences(value *core.ArtifactRef) error {
	return core.ValidateArtifactRef(value, "references")
}

func validateArtifactRelations(value *core.ArtifactRef) error {
	return core.ValidateArtifactRef(value, "relations")
}

func validateLowerToken(value, label string, maximum int) error {
	return contractwire.ValidateLowerToken(value, label, maximum)
}

func validateSchemaName(value, label string) error {
	return contractwire.ValidateSchemaName(value, label)
}

func validateHash(value, label string) error {
	return contractwire.ValidateHash(value, label)
}

func validateText(value, label string, maximum int, nonempty bool) error {
	return contractwire.ValidateText(value, label, maximum, nonempty)
}

func validateUnixMS(value int64, label string) error {
	return contractwire.ValidateUnixMS(value, label)
}

func reject(code core.RejectionCode, detail string) error {
	return &core.ContractError{Code: code, Detail: detail}
}

func rejectf(code core.RejectionCode, format string, arguments ...any) error {
	return reject(code, fmt.Sprintf(format, arguments...))
}

func withRejection(err error, fallback core.RejectionCode) error {
	if err == nil {
		return nil
	}
	if _, ok := core.RejectionCodeOf(err); ok {
		return err
	}
	return reject(fallback, err.Error())
}

func observationDigest(domain string, canonical []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(canonical)
	return hex.EncodeToString(hasher.Sum(nil))
}
