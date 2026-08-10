package cognitiveatomcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
)

func DecodeAtom(data []byte) (*CognitiveAtom, error) {
	node, err := parseStrictJSONBounded(data, maxAtomBytes)
	if err != nil {
		return nil, fmt.Errorf("CognitiveAtom JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("CognitiveAtom root must be an object")
	}
	if err := validateAtomShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("CognitiveAtom input is not exact compact canonical JSON")
	}
	var atom CognitiveAtom
	if err := decodeStrictTyped(data, &atom); err != nil {
		return nil, fmt.Errorf("CognitiveAtom: %w", err)
	}
	if err := validateAtom(&atom); err != nil {
		return nil, fmt.Errorf("CognitiveAtom: %w", err)
	}
	state, err := canonicalStateFromTyped(&atom)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(state.atom, canonical) {
		return nil, fmt.Errorf("typed CognitiveAtom does not preserve exact canonical input")
	}
	if atom.Integrity.CanonicalSHA256 != state.digest {
		return nil, fmt.Errorf("canonical_sha256 mismatch: got %q want %q", atom.Integrity.CanonicalSHA256, state.digest)
	}
	applyCanonicalState(&atom, state)
	return &atom, nil
}

// Decode is an alias for DecodeAtom.
func Decode(data []byte) (*CognitiveAtom, error) { return DecodeAtom(data) }

func decodeStrictTyped(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return fmt.Errorf("unexpected trailing JSON value")
	}
	return nil
}

type canonicalState struct {
	atom    []byte
	payload []byte
	digest  string
}

func canonicalStateFromTyped(atom *CognitiveAtom) (canonicalState, error) {
	encoded, err := encodeTypedJSON(atom)
	if err != nil {
		return canonicalState{}, err
	}
	node, err := parseStrictJSONBounded(encoded, maxAtomBytes)
	if err != nil {
		return canonicalState{}, err
	}
	root, ok := node.(map[string]any)
	if !ok {
		return canonicalState{}, fmt.Errorf("typed CognitiveAtom did not encode as an object")
	}
	if err := validateAtomShape(root); err != nil {
		return canonicalState{}, err
	}
	atomJSON, err := canonicalJSON(root)
	if err != nil {
		return canonicalState{}, err
	}
	if len(atomJSON) > maxAtomBytes {
		return canonicalState{}, fmt.Errorf("canonical atom exceeds %d bytes", maxAtomBytes)
	}
	payloadNode := cloneNode(root).(map[string]any)
	payloadNode["integrity"].(map[string]any)["canonical_sha256"] = ""
	payload, err := canonicalJSON(payloadNode)
	if err != nil {
		return canonicalState{}, err
	}
	if len(payload)+sha256.Size*2 > maxAtomBytes {
		return canonicalState{}, fmt.Errorf("canonical sealed atom exceeds %d bytes", maxAtomBytes)
	}
	hasher := sha256.New()
	hasher.Write([]byte(atomDigestDomain))
	hasher.Write([]byte{0})
	hasher.Write(payload)
	return canonicalState{atom: atomJSON, payload: payload, digest: hex.EncodeToString(hasher.Sum(nil))}, nil
}

func verifiedCanonicalState(atom *CognitiveAtom) (canonicalState, error) {
	if err := validateAtom(atom); err != nil {
		return canonicalState{}, err
	}
	state, err := canonicalStateFromTyped(atom)
	if err != nil {
		return canonicalState{}, err
	}
	if atom.Integrity.CanonicalSHA256 != state.digest {
		return canonicalState{}, fmt.Errorf("canonical_sha256 mismatch: got %q want %q", atom.Integrity.CanonicalSHA256, state.digest)
	}
	return state, nil
}

func applyCanonicalState(atom *CognitiveAtom, state canonicalState) {
	atom.canonicalAtom = state.atom
	atom.canonicalPayload = state.payload
	atom.digest = state.digest
}

func sealAtom(atom *CognitiveAtom) error {
	atom.Integrity.CanonicalSHA256 = string(bytes.Repeat([]byte{'0'}, sha256.Size*2))
	if err := validateAtom(atom); err != nil {
		return err
	}
	state, err := canonicalStateFromTyped(atom)
	if err != nil {
		return err
	}
	atom.Integrity.CanonicalSHA256 = state.digest
	state, err = verifiedCanonicalState(atom)
	if err != nil {
		return err
	}
	applyCanonicalState(atom, state)
	return nil
}
