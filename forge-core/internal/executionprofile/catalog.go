// Package executionprofile resolves Project-scoped Conversation work to a
// server-owned, content-pinned execution profile. It does not grant consent or
// start execution.
package executionprofile

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"io"
	"strings"
	"unicode/utf8"
)

var (
	ErrInvalidPolicy         = errors.New("execution profile policy is invalid")
	ErrProfileUnavailable    = errors.New("execution profile is unavailable")
	ErrInvalidProjectContext = errors.New("Hub returned invalid Project context")
)

const maxBindings = 256
const maxBindingConfigBytes = 4096

// Binding pins one Hub Project to a profile selected by trusted server policy.
// The digest is the SHA-256 of the server's canonical profile manifest.
type Binding struct {
	ProjectID string
	Profile   intentmodel.ServerExecutionProfile
}

// ParseBindingConfig decodes one exact, bounded server-startup policy record:
// {"project_id":"...","profile_id":"...","profile_sha256":"64 lowercase hex"}.
// The manifest digest is a server policy pin; this function does not load or
// interpret execution configuration.
func ParseBindingConfig(encoded string) (Binding, error) {
	if len(encoded) == 0 || len(encoded) > maxBindingConfigBytes || !utf8.ValidString(encoded) {
		return Binding{}, ErrInvalidPolicy
	}
	decoder := json.NewDecoder(bytes.NewBufferString(encoded))
	opening, err := decoder.Token()
	if err != nil || opening != json.Delim('{') {
		return Binding{}, ErrInvalidPolicy
	}
	fields := make(map[string]string, 3)
	for decoder.More() {
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok {
			return Binding{}, ErrInvalidPolicy
		}
		if _, exists := fields[key]; exists {
			return Binding{}, ErrInvalidPolicy
		}
		var value string
		if err := decoder.Decode(&value); err != nil {
			return Binding{}, ErrInvalidPolicy
		}
		fields[key] = value
	}
	closing, err := decoder.Token()
	if err != nil || closing != json.Delim('}') {
		return Binding{}, ErrInvalidPolicy
	}
	if _, err := decoder.Token(); !errors.Is(err, io.EOF) || len(fields) != 3 {
		return Binding{}, ErrInvalidPolicy
	}
	projectID, hasProjectID := fields["project_id"]
	profileID, hasProfileID := fields["profile_id"]
	digestText, hasDigest := fields["profile_sha256"]
	if !hasProjectID || !hasProfileID || !hasDigest || !validID(projectID) || !validID(profileID) || len(digestText) != 64 {
		return Binding{}, ErrInvalidPolicy
	}
	digestBytes, err := hex.DecodeString(digestText)
	if err != nil || hex.EncodeToString(digestBytes) != digestText || len(digestBytes) != 32 {
		return Binding{}, ErrInvalidPolicy
	}
	var digest [32]byte
	copy(digest[:], digestBytes)
	return Binding{ProjectID: projectID, Profile: intentmodel.ServerExecutionProfile{ID: profileID, SHA256: digest}}, nil
}

// ParseBindingConfigs parses a bounded list and rejects duplicate Project
// policy entries before a server starts.
func ParseBindingConfigs(values []string) ([]Binding, error) {
	if len(values) > maxBindings {
		return nil, ErrInvalidPolicy
	}
	bindings := make([]Binding, 0, len(values))
	for _, value := range values {
		binding, err := ParseBindingConfig(value)
		if err != nil {
			return nil, err
		}
		bindings = append(bindings, binding)
	}
	if _, err := New(bindings); err != nil {
		return nil, err
	}
	return bindings, nil
}

// Catalog is an immutable allowlist. An empty catalog fails closed.
type Catalog struct {
	byProject map[string]intentmodel.ServerExecutionProfile
}

type projectConversationReader interface {
	OwnedProjectConversationIdentity(
		context.Context,
		model.Owner,
		string,
	) (model.OwnedProjectConversationIdentity, error)
}

// New builds a copy of the server-owned Project/profile policy and rejects
// duplicate or malformed bindings before the server can use it.
func New(bindings []Binding) (*Catalog, error) {
	if len(bindings) > maxBindings {
		return nil, ErrInvalidPolicy
	}
	profiles := make(map[string]intentmodel.ServerExecutionProfile, len(bindings))
	for _, binding := range bindings {
		if !validID(binding.ProjectID) || !validID(binding.Profile.ID) {
			return nil, ErrInvalidPolicy
		}
		if _, exists := profiles[binding.ProjectID]; exists {
			return nil, ErrInvalidPolicy
		}
		profiles[binding.ProjectID] = binding.Profile
	}
	return &Catalog{byProject: profiles}, nil
}

// ResolveConversationProfile derives Project identity from the owner-filtered
// Hub record, then selects a profile from this immutable server catalog. The
// request supplies neither Project ID nor profile ID/digest.
func (catalog *Catalog) ResolveConversationProfile(
	ctx context.Context,
	reader projectConversationReader,
	owner model.Owner,
	conversationID string,
) (intentmodel.ServerExecutionProfile, error) {
	if catalog == nil || len(catalog.byProject) == 0 {
		return intentmodel.ServerExecutionProfile{}, ErrProfileUnavailable
	}
	if ctx == nil || reader == nil || !validOwner(owner) || !validID(conversationID) {
		return intentmodel.ServerExecutionProfile{}, ErrInvalidPolicy
	}
	identity, err := reader.OwnedProjectConversationIdentity(ctx, owner, conversationID)
	if err != nil {
		return intentmodel.ServerExecutionProfile{}, err
	}
	if identity.ConversationID != conversationID || !validID(identity.ProjectID) {
		return intentmodel.ServerExecutionProfile{}, ErrInvalidProjectContext
	}
	return catalog.ProfileForProject(identity.ProjectID)
}

// ProfileForProject returns the immutable server-owned binding for a Project
// ID already derived from an owner-filtered Hub record.
func (catalog *Catalog) ProfileForProject(projectID string) (intentmodel.ServerExecutionProfile, error) {
	if catalog == nil || !validID(projectID) {
		return intentmodel.ServerExecutionProfile{}, ErrProfileUnavailable
	}
	profile, exists := catalog.byProject[projectID]
	if !exists {
		return intentmodel.ServerExecutionProfile{}, ErrProfileUnavailable
	}
	return profile, nil
}

func validOwner(owner model.Owner) bool {
	return validBoundedText(owner.Issuer, 2048) && validBoundedText(owner.Subject, 255) &&
		validBoundedText(owner.TenantID, 256)
}

func validID(value string) bool {
	return validBoundedText(value, 128)
}

func validBoundedText(value string, maximum int) bool {
	return strings.TrimSpace(value) != "" && len(value) <= maximum &&
		!strings.ContainsAny(value, "\x00\r\n")
}
