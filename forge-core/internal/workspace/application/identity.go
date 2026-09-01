package application

import (
	"crypto/rand"
	"fmt"
	"strings"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const crockfordAlphabet = "0123456789abcdefghjkmnpqrstvwxyz"

func randomPlatformID(prefix string) (string, error) {
	var entropy [16]byte
	if _, err := rand.Read(entropy[:]); err != nil {
		return "", fmt.Errorf("generate Workspace identity: %w", err)
	}
	return prefix + "_" + encodeCrockford(entropy), nil
}

func encodeCrockford(entropy [16]byte) string {
	encoded := make([]byte, 26)
	var buffer uint32
	bits, position := uint(2), 0
	for _, value := range entropy {
		buffer = buffer<<8 | uint32(value)
		bits += 8
		for bits >= 5 {
			bits -= 5
			encoded[position] = crockfordAlphabet[(buffer>>bits)&31]
			position++
		}
		if bits == 0 {
			buffer = 0
		} else {
			buffer &= 1<<bits - 1
		}
	}
	return string(encoded)
}

func messageForEvent(eventID string) (string, error) {
	if err := core.ValidatePlatformID(eventID); err != nil || !strings.HasPrefix(eventID, "evt_") {
		return "", fmt.Errorf("event identity source returned an invalid evt_ Platform ID")
	}
	return "msg" + eventID[3:], nil
}
