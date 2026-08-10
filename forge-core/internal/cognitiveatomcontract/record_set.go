package cognitiveatomcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"
)

func DecodeAtomSet(data []byte) ([]*CognitiveAtom, error) {
	node, err := parseStrictJSONBounded(data, maxSetBytes)
	if err != nil {
		return nil, fmt.Errorf("CognitiveAtom set JSON: %w", err)
	}
	array, ok := node.([]any)
	if !ok || len(array) == 0 {
		return nil, fmt.Errorf("CognitiveAtom set must be a nonempty array")
	}
	canonical, err := canonicalJSON(array)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("CognitiveAtom set input is not exact compact canonical JSON")
	}
	atoms := make([]*CognitiveAtom, 0, len(array))
	for index, node := range array {
		encoded, err := canonicalJSON(node)
		if err != nil {
			return nil, fmt.Errorf("atom %d: %w", index, err)
		}
		if len(encoded) > maxAtomBytes {
			return nil, fmt.Errorf("atom %d exceeds %d bytes", index, maxAtomBytes)
		}
		atom, err := DecodeAtom(encoded)
		if err != nil {
			return nil, fmt.Errorf("atom %d: %w", index, err)
		}
		atoms = append(atoms, atom)
	}
	if err := ValidateAtomSet(atoms); err != nil {
		return nil, err
	}
	return atoms, nil
}

func ValidateAtomSet(atoms []*CognitiveAtom) error {
	if len(atoms) == 0 || len(atoms) > maxArrayItems {
		return fmt.Errorf("CognitiveAtom set must contain 1..%d atoms", maxArrayItems)
	}
	previousID := ""
	totalBytes := len(atoms) + 1
	for index, atom := range atoms {
		state, err := verifiedCanonicalState(atom)
		if err != nil {
			return fmt.Errorf("atom %d: %w", index, err)
		}
		if index > 0 && atom.Metadata.AtomID <= previousID {
			return fmt.Errorf("CognitiveAtom set must be sorted by unique atom_id")
		}
		totalBytes += len(state.atom)
		previousID = atom.Metadata.AtomID
	}
	if totalBytes > maxSetBytes {
		return fmt.Errorf("CognitiveAtom set exceeds %d canonical bytes", maxSetBytes)
	}
	return nil
}

// CanonicalAtomSetJSON returns the exact canonical array encoding. The caller
// must provide atoms in strictly increasing atom_id order.
func CanonicalAtomSetJSON(atoms []*CognitiveAtom) ([]byte, error) {
	if err := ValidateAtomSet(atoms); err != nil {
		return nil, err
	}
	buffer := bytes.NewBuffer(make([]byte, 0, 2+len(atoms)*256))
	buffer.WriteByte('[')
	for index, atom := range atoms {
		if index > 0 {
			buffer.WriteByte(',')
		}
		buffer.Write(atom.AtomJSON())
	}
	buffer.WriteByte(']')
	if buffer.Len() > maxSetBytes {
		return nil, fmt.Errorf("CognitiveAtom set exceeds %d canonical bytes", maxSetBytes)
	}
	return buffer.Bytes(), nil
}

// AtomSetDigest returns the domain-separated digest over the complete sealed
// canonical atom-set bytes.
func AtomSetDigest(atoms []*CognitiveAtom) (string, error) {
	encoded, err := CanonicalAtomSetJSON(atoms)
	if err != nil {
		return "", err
	}
	return domainDigest(atomSetDigestDomain, encoded), nil
}

func sortAtoms(atoms []*CognitiveAtom) {
	sort.Slice(atoms, func(left, right int) bool {
		return atoms[left].Metadata.AtomID < atoms[right].Metadata.AtomID
	})
}

func domainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	hasher.Write([]byte(domain))
	hasher.Write([]byte{0})
	hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}
