// Package runnertransport contains the authority-neutral wire verifier for
// the Forge Runner control-plane channel. It matches the small D3 envelope
// used by the ecosystem Runner: a method/path/timestamp/nonce binding, an
// HMAC-SHA256 signature, and a bounded JSON object payload.
//
// Verification is deliberately side-effect free apart from an explicitly
// supplied in-memory replay cache. It does not register a device, read or
// write credentials, accept a heartbeat, issue a lease, select a target, or
// dispatch work. A production HTTP adapter must still be mounted only after
// the device-fabric activation gate and its persistence/authentication review
// are accepted.
package runnertransport

import (
	"bytes"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"regexp"
	"sort"
	"strings"
	"sync"
	"unicode/utf8"
)

const (
	// SchemaVersion identifies the bounded transport-admission envelope.
	SchemaVersion = "forge.runner-transport-admission/v1"
	// EvaluationMode states that this package verifies transport bytes only.
	EvaluationMode = "pure_runner_transport_admission"

	// TimestampWindowSeconds follows the ecosystem D3 Runner contract.
	TimestampWindowSeconds int64 = 300
	// DefaultReplayCacheSize bounds the default nonce cache.
	DefaultReplayCacheSize = 10_000
	MaxReplayCacheSize     = 1 << 20
	MaxNonceBytes          = 128
	MaxMethodBytes         = 16
	MaxPathBytes           = 2_048
	MaxSecretBytes         = 512
	MaxPayloadBytes        = 1 << 20
	MaxSignatureBytes      = sha256.Size * 2
)

// Error is a stable verifier rejection code.
type Error string

const (
	ErrInvalidSecret        Error = "invalid_secret"
	ErrInvalidMethod        Error = "invalid_method"
	ErrInvalidPath          Error = "invalid_path"
	ErrInvalidTimestamp     Error = "invalid_timestamp"
	ErrStaleTimestamp       Error = "stale_timestamp"
	ErrInvalidNonce         Error = "invalid_nonce"
	ErrReplay               Error = "replay"
	ErrInvalidSignature     Error = "invalid_signature"
	ErrInvalidPayload       Error = "invalid_payload"
	ErrPayloadTooLarge      Error = "payload_too_large"
	ErrMalformedEnvelope    Error = "malformed_envelope"
	ErrUnknownEnvelopeKey   Error = "unknown_envelope_key"
	ErrDuplicateEnvelopeKey Error = "duplicate_envelope_key"
	ErrTrailingJSON         Error = "trailing_json"
)

func (err Error) Error() string { return string(err) }

// Envelope is the JSON body signed by a Runner. Payload must be one compact,
// canonical JSON object. RawMessage is used so signature verification covers
// exactly the bytes received rather than a re-encoded approximation.
type Envelope struct {
	TS      int64           `json:"ts"`
	Nonce   string          `json:"nonce"`
	Sig     string          `json:"sig"`
	Payload json.RawMessage `json:"payload"`
}

// Authority is fixed false for this package. It is included in contract
// projections so a future adapter cannot mistake signature verification for
// device or execution authority.
type Authority struct {
	IdentityVerified    bool `json:"identity_verified"`
	HeartbeatAccepted   bool `json:"heartbeat_accepted"`
	LeaseIssued         bool `json:"lease_issued"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// Observation is the metadata-only result of successful verification.
type Observation struct {
	SchemaVersion  string    `json:"schema_version"`
	EvaluationMode string    `json:"evaluation_mode"`
	Method         string    `json:"method"`
	Path           string    `json:"path"`
	Timestamp      int64     `json:"timestamp"`
	Nonce          string    `json:"nonce"`
	PayloadSHA256  string    `json:"payload_sha256"`
	PayloadBytes   int       `json:"payload_bytes"`
	ReplayChecked  bool      `json:"replay_checked"`
	PreviewOnly    bool      `json:"preview_only"`
	Authority      Authority `json:"authority"`
}

// ReplayCache is a bounded process-local nonce set. It intentionally stores
// no device secret or payload. A cache is optional for pure signature checks;
// production adapters should always provide one that is scoped to the
// authenticated Runner identity and process lifetime.
type ReplayCache struct {
	mu      sync.Mutex
	entries map[string]int64
	max     int
}

// NewReplayCache returns a bounded cache. Non-positive sizes use the default.
func NewReplayCache(max int) *ReplayCache {
	if max <= 0 || max > MaxReplayCacheSize {
		max = DefaultReplayCacheSize
	}
	return &ReplayCache{entries: make(map[string]int64, max), max: max}
}

// CheckAndRemember rejects a nonce already seen and remembers a new one.
func (cache *ReplayCache) CheckAndRemember(nonce string, timestamp int64) error {
	if cache == nil {
		return nil
	}
	if err := validateNonce(nonce); err != nil {
		return err
	}
	cache.mu.Lock()
	defer cache.mu.Unlock()
	if _, exists := cache.entries[nonce]; exists {
		return ErrReplay
	}
	if len(cache.entries) >= cache.max {
		// The D3 contract bounds memory. Clearing the cache is an explicit
		// cold-start tradeoff; timestamp and HMAC checks still apply.
		cache.entries = make(map[string]int64, cache.max)
	}
	cache.entries[nonce] = timestamp
	return nil
}

// Sign computes the D3 HMAC-SHA256 signature over
// `method|path|ts|nonce` followed by the exact canonical payload bytes.
func Sign(secret, method, path string, timestamp int64, nonce string, payload []byte) (string, error) {
	if err := validateSecret(secret); err != nil {
		return "", err
	}
	if err := validateMethod(method); err != nil {
		return "", err
	}
	if err := validatePath(path); err != nil {
		return "", err
	}
	if timestamp <= 0 {
		return "", ErrInvalidTimestamp
	}
	if err := validateNonce(nonce); err != nil {
		return "", err
	}
	canonical, err := canonicalPayload(payload)
	if err != nil {
		return "", err
	}
	message := []byte(fmt.Sprintf("%s|%s|%d|%s", method, path, timestamp, nonce))
	message = append(message, canonical...)
	hash := hmac.New(sha256.New, []byte(secret))
	_, _ = hash.Write(message)
	return hex.EncodeToString(hash.Sum(nil)), nil
}

// Verify authenticates one envelope against explicit method/path/time. It
// returns a metadata-only observation and never interprets the payload.
func Verify(secret, method, path string, envelope Envelope, nowUnixSeconds int64, replay *ReplayCache) (Observation, error) {
	if err := validateSecret(secret); err != nil {
		return Observation{}, err
	}
	if err := validateMethod(method); err != nil {
		return Observation{}, err
	}
	if err := validatePath(path); err != nil {
		return Observation{}, err
	}
	if nowUnixSeconds <= 0 || envelope.TS <= 0 {
		return Observation{}, ErrInvalidTimestamp
	}
	if envelope.TS > nowUnixSeconds {
		if envelope.TS-nowUnixSeconds > TimestampWindowSeconds {
			return Observation{}, ErrStaleTimestamp
		}
	} else if nowUnixSeconds-envelope.TS > TimestampWindowSeconds {
		return Observation{}, ErrStaleTimestamp
	}
	if err := validateNonce(envelope.Nonce); err != nil {
		return Observation{}, err
	}
	canonical, err := canonicalPayload(envelope.Payload)
	if err != nil {
		return Observation{}, err
	}
	expected, err := Sign(secret, method, path, envelope.TS, envelope.Nonce, canonical)
	if err != nil {
		return Observation{}, err
	}
	if len(envelope.Sig) != MaxSignatureBytes || !isLowerHex(envelope.Sig) ||
		!hmac.Equal([]byte(expected), []byte(envelope.Sig)) {
		return Observation{}, ErrInvalidSignature
	}
	if replay != nil {
		if err := replay.CheckAndRemember(envelope.Nonce, envelope.TS); err != nil {
			return Observation{}, err
		}
	}
	digest := sha256.Sum256(canonical)
	return Observation{
		SchemaVersion: SchemaVersion, EvaluationMode: EvaluationMode,
		Method: method, Path: path, Timestamp: envelope.TS, Nonce: envelope.Nonce,
		PayloadSHA256: hex.EncodeToString(digest[:]), PayloadBytes: len(canonical),
		ReplayChecked: replay != nil, PreviewOnly: true, Authority: Authority{},
	}, nil
}

// DecodeEnvelope strictly decodes an outer JSON envelope. It rejects unknown
// and duplicate keys, trailing values, and payloads that are not canonical
// JSON objects. The payload is left opaque to this transport layer.
func DecodeEnvelope(data []byte) (Envelope, error) {
	if len(data) == 0 {
		return Envelope{}, ErrMalformedEnvelope
	}
	if len(data) > MaxPayloadBytes+4*MaxNonceBytes+512 {
		return Envelope{}, ErrPayloadTooLarge
	}
	if !utf8.Valid(data) {
		return Envelope{}, ErrMalformedEnvelope
	}
	if err := rejectDuplicateKeys(data); err != nil {
		return Envelope{}, err
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil || raw == nil {
		return Envelope{}, ErrMalformedEnvelope
	}
	want := []string{"ts", "nonce", "sig", "payload"}
	if len(raw) != len(want) {
		for key := range raw {
			if !contains(want, key) {
				return Envelope{}, ErrUnknownEnvelopeKey
			}
		}
		return Envelope{}, ErrMalformedEnvelope
	}
	for _, key := range want {
		if _, ok := raw[key]; !ok {
			return Envelope{}, ErrMalformedEnvelope
		}
	}
	var envelope Envelope
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&envelope); err != nil {
		return Envelope{}, ErrMalformedEnvelope
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		return Envelope{}, ErrTrailingJSON
	}
	if err := validateNonce(envelope.Nonce); err != nil {
		return Envelope{}, err
	}
	if _, err := canonicalPayload(envelope.Payload); err != nil {
		return Envelope{}, err
	}
	return envelope, nil
}

func canonicalPayload(payload []byte) ([]byte, error) {
	if len(payload) == 0 || len(payload) > MaxPayloadBytes || !utf8.Valid(payload) {
		if len(payload) > MaxPayloadBytes {
			return nil, ErrPayloadTooLarge
		}
		return nil, ErrInvalidPayload
	}
	var value map[string]any
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.UseNumber()
	if err := decoder.Decode(&value); err != nil || value == nil {
		return nil, ErrInvalidPayload
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		return nil, ErrTrailingJSON
	}
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, ErrInvalidPayload
	}
	canonical := bytes.TrimSuffix(buffer.Bytes(), []byte{'\n'})
	if !bytes.Equal(canonical, payload) {
		return nil, ErrInvalidPayload
	}
	return append([]byte(nil), canonical...), nil
}

func validateSecret(secret string) error {
	if secret == "" || len(secret) > MaxSecretBytes || !utf8.ValidString(secret) {
		return ErrInvalidSecret
	}
	return nil
}

var methodPattern = regexp.MustCompile(`^[A-Z]{1,16}$`)

func validateMethod(method string) error {
	if len(method) > MaxMethodBytes || !methodPattern.MatchString(method) {
		return ErrInvalidMethod
	}
	return nil
}

func validatePath(path string) error {
	if path == "" || len(path) > MaxPathBytes || !strings.HasPrefix(path, "/") ||
		strings.ContainsAny(path, "\x00\r\n") || !utf8.ValidString(path) {
		return ErrInvalidPath
	}
	return nil
}

func validateNonce(nonce string) error {
	if nonce == "" || len(nonce) > MaxNonceBytes || !utf8.ValidString(nonce) {
		return ErrInvalidNonce
	}
	for _, character := range nonce {
		if !(character >= 'A' && character <= 'Z' || character >= 'a' && character <= 'z' || character >= '0' && character <= '9' || character == '-' || character == '_' || character == '.') {
			return ErrInvalidNonce
		}
	}
	return nil
}

func isLowerHex(value string) bool {
	for _, character := range value {
		if !(character >= '0' && character <= '9' || character >= 'a' && character <= 'f') {
			return false
		}
	}
	return true
}

func contains(values []string, value string) bool {
	for _, item := range values {
		if item == value {
			return true
		}
	}
	return false
}

func rejectDuplicateKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkJSON(decoder); err != nil {
		return ErrMalformedEnvelope
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return ErrTrailingJSON
		}
		return ErrMalformedEnvelope
	}
	return nil
}

func walkJSON(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delim, ok := token.(json.Delim)
	if !ok {
		return nil
	}
	switch delim {
	case '{':
		seen := map[string]struct{}{}
		for decoder.More() {
			key, err := decoder.Token()
			if err != nil {
				return err
			}
			name, ok := key.(string)
			if !ok {
				return ErrMalformedEnvelope
			}
			if _, exists := seen[name]; exists {
				return ErrDuplicateEnvelopeKey
			}
			seen[name] = struct{}{}
			if err := walkJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	case '[':
		for decoder.More() {
			if err := walkJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	default:
		return ErrMalformedEnvelope
	}
}

// SortedAuthorityKeys is useful to contract tests that render the fixed
// authority boundary without relying on Go struct field order.
func SortedAuthorityKeys() []string {
	keys := []string{"identity_verified", "heartbeat_accepted", "lease_issued", "reservation_created", "execution_authorized", "dispatch_performed", "audit_published"}
	sort.Strings(keys)
	return keys
}
