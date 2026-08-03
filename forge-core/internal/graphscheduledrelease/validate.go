package graphscheduledrelease

import (
	"math"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateReleaseControl(value ReleaseControl) error {
	if !validControlHeader(value) || validateGraphSource(value) != nil ||
		validateScheduleSource(value) != nil || validateContractSource(value) != nil ||
		validateProviderSource(value) != nil {
		return errInvalidControl
	}
	digest, err := domainDigest(releaseControlDigestDomain, releasePayload(value))
	if err != nil || digest != value.SnapshotSHA256 {
		return errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxReleaseControlBytes {
		return errInvalidControl
	}
	return nil
}

func validControlHeader(value ReleaseControl) bool {
	return value.V == ReleaseControlVersion && value.SchedulerProtocolVersion == 1 &&
		value.ReleaseControlProtocolVersion == ReleaseControlProtocol &&
		value.JournalEvents != nil && len(value.JournalEvents) == 1 &&
		isLowerHexDigest(value.SnapshotSHA256)
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
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" &&
		len(value) <= maximum && !strings.ContainsFunc(value, unsupportedCharacter)
}

func unsupportedCharacter(value rune) bool {
	return unicode.IsControl(value) || value == '\u061c' || value == '\u200e' ||
		value == '\u200f' || value >= '\u2028' && value <= '\u202e' ||
		value >= '\u2066' && value <= '\u2069'
}

func validSignedTime(value uint64) bool {
	return value <= math.MaxInt64
}
