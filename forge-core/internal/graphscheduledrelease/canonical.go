package graphscheduledrelease

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
)

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidControl
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidControl
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", errInvalidControl
	}
	return rawDomainDigest(domain, encoded), nil
}

func rawDomainDigest(domain string, value []byte) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(value)
	return hex.EncodeToString(digest.Sum(nil))
}

func releasePayload(value ReleaseControl) releaseControlPayload {
	return releaseControlPayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: value.ReleaseControlProtocolVersion,
		GraphRun:                      value.GraphRun, JournalEvents: value.JournalEvents,
		ControlSnapshot: value.ControlSnapshot, ScheduleRecord: value.ScheduleRecord,
		Schedule: value.Schedule, ScheduledContractRecord: value.ScheduledContractRecord,
		ScheduledContract: value.ScheduledContract, ProviderRequest: value.ProviderRequest,
		ProviderRequestJSON: value.ProviderRequestJSON,
	}
}

// MarshalAuthorization returns exact compact canonical JSON without a newline.
func MarshalAuthorization(value Authorization) ([]byte, error) {
	if validateAuthorization(value) != nil {
		return nil, errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > maxAuthorizationBytes {
		return nil, errInvalidControl
	}
	return encoded, nil
}
