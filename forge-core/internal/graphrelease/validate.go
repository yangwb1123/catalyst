package graphrelease

import (
	"encoding/json"
	"math"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/graphplan"
)

type journalFacts struct {
	Prepared       preparedEvent
	Contract       contractEvent
	Dispatch       dispatchEvent
	PreparedSHA256 string
	ContractSHA256 string
	DispatchSHA256 string
	Bytes          uint64
}

func validateReleaseControl(control ReleaseControl) error {
	_, err := validateReleaseControlFacts(control)
	return err
}

func validateReleaseControlFacts(control ReleaseControl) (journalFacts, error) {
	if !validReleaseHeader(control) {
		return journalFacts{}, errInvalidControl
	}
	planJSON, err := graphplan.MarshalPlan(control.Plan)
	if err != nil || len(planJSON) == 0 || len(planJSON) > graphplan.MaxSpecBytes {
		return journalFacts{}, errInvalidControl
	}
	facts, err := decodeJournal(control.JournalEvents)
	if err != nil || validateGraphRun(control, facts, planJSON) != nil ||
		validateContractBindings(control, facts) != nil ||
		validateDispatchBindings(control, facts) != nil {
		return journalFacts{}, errInvalidControl
	}
	digest, err := domainDigest(releaseControlDigestDomain, releasePayload(control))
	if err != nil || digest != control.SnapshotSHA256 {
		return journalFacts{}, errInvalidControl
	}
	encoded, err := canonicalBytes(control)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxReleaseControlBytes {
		return journalFacts{}, errInvalidControl
	}
	return facts, nil
}

func validReleaseHeader(control ReleaseControl) bool {
	return control.V == ReleaseControlVersion &&
		control.SchedulerProtocolVersion == graphplan.SchedulerProtocolVersion &&
		control.ReleaseControlProtocolVersion == ReleaseControlProtocol &&
		control.JournalEvents != nil && len(control.JournalEvents) == 3 &&
		isLowerHexDigest(control.SnapshotSHA256)
}

func decodeJournal(events []json.RawMessage) (journalFacts, error) {
	if len(events) != 3 {
		return journalFacts{}, errInvalidControl
	}
	for _, event := range events {
		if len(event) == 0 || len(event) > maxGraphEventBytes {
			return journalFacts{}, errInvalidControl
		}
	}
	prepared, err := decodeExact[preparedEvent](events[0])
	if err != nil {
		return journalFacts{}, errInvalidControl
	}
	contract, err := decodeExact[contractEvent](events[1])
	if err != nil {
		return journalFacts{}, errInvalidControl
	}
	dispatch, err := decodeExact[dispatchEvent](events[2])
	if err != nil {
		return journalFacts{}, errInvalidControl
	}
	total := len(events[0]) + len(events[1]) + len(events[2])
	if total > maxGraphJournalBytes {
		return journalFacts{}, errInvalidControl
	}
	return journalFacts{
		Prepared: prepared, Contract: contract, Dispatch: dispatch,
		PreparedSHA256: rawDomainDigest(preparedEventDigestDomain, events[0]),
		ContractSHA256: rawDomainDigest(controlEventDigestDomain, events[1]),
		DispatchSHA256: rawDomainDigest(controlEventDigestDomain, events[2]),
		Bytes:          uint64(total),
	}, nil
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') &&
			(character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func validText(value string, maximum int) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" && len(value) <= maximum &&
		len(value) > 0 && !hasUnsupportedText(value)
}

func hasUnsupportedText(value string) bool {
	for _, character := range value {
		if unicode.IsControl(character) || character == '\u061c' ||
			character == '\u200e' || character == '\u200f' ||
			character >= '\u2028' && character <= '\u202e' ||
			character >= '\u2066' && character <= '\u2069' {
			return true
		}
	}
	return false
}

func validSignedTime(value uint64) bool {
	return value <= math.MaxInt64
}
