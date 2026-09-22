// Package aeroidprofile contains the pure, caller-supplied Aero-ID profile
// projection used by Forge clients.  It deliberately has no HTTP client,
// storage, clock, or authorization integration.
package aeroidprofile

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	SchemaVersion  = "forge.aero-id-profile-projection/v1"
	EvaluationMode = "pure_projection_only"
	Source         = "aero-id"
	Notice         = "This is a caller-supplied Aero-ID profile/membership projection. Owner, account, profile, membership, consistency, and status values are unverified; it grants no Forge authorization, device, reservation, scheduling, dispatch, or execution authority."
	MaxBytes       = 256 * 1024
	MaxMemberships = 128
)

// Owner is the exact Snaplink provenance tuple that a future authenticated
// integration must bind.  In this contract it is still caller-supplied and
// unverified.
type Owner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

type Profile struct {
	AccountID   string `json:"account_id"`
	DisplayName string `json:"display_name"`
	AvatarURL   string `json:"avatar_url"`
	Locale      string `json:"locale"`
	Timezone    string `json:"timezone"`
}

// Membership is deliberately source-scoped.  It must not be collapsed into a
// Forge tenant or authorization grant by a consumer.
type Membership struct {
	Source    string `json:"source"`
	ScopeType string `json:"scope_type"`
	ScopeID   string `json:"scope_id"`
	Role      string `json:"role"`
	Status    string `json:"status"`
}

type Authority struct {
	IdentityVerified        bool `json:"identity_verified"`
	ProfileAuthoritative    bool `json:"profile_authoritative"`
	MembershipAuthoritative bool `json:"membership_authoritative"`
	AuthorizationGranted    bool `json:"authorization_granted"`
	ExecutionAuthorized     bool `json:"execution_authorized"`
	ReservationCreated      bool `json:"reservation_created"`
	DispatchPerformed       bool `json:"dispatch_performed"`
}

type Projection struct {
	SchemaVersion                  string       `json:"schema_version"`
	EvaluationMode                 string       `json:"evaluation_mode"`
	Source                         string       `json:"source"`
	OwnerDeclaration               Owner        `json:"owner_declaration"`
	OwnerDeclarationUnverified     bool         `json:"owner_declaration_unverified"`
	ProfileAttributesUnverified    bool         `json:"profile_attributes_unverified"`
	MembershipAttributesUnverified bool         `json:"membership_attributes_unverified"`
	Profile                        Profile      `json:"profile"`
	Memberships                    []Membership `json:"memberships"`
	Consistency                    string       `json:"consistency"`
	Partial                        bool         `json:"partial"`
	Notice                         string       `json:"notice"`
	Authority                      Authority    `json:"authority"`
}

var errTrailingJSON = errors.New("aero-id profile projection has trailing JSON")

// Decode validates one bounded projection and rejects unknown or duplicate
// JSON fields.  It reads only the supplied bytes.
func Decode(data []byte) (Projection, error) {
	if len(data) == 0 || len(data) > MaxBytes {
		return Projection{}, fmt.Errorf("aero-id profile projection size is invalid")
	}
	if !utf8.Valid(data) {
		return Projection{}, fmt.Errorf("aero-id profile projection is not valid UTF-8")
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return Projection{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var projection Projection
	if err := decoder.Decode(&projection); err != nil {
		return Projection{}, fmt.Errorf("decode aero-id profile projection: %w", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return Projection{}, errTrailingJSON
		}
		return Projection{}, fmt.Errorf("%w: %v", errTrailingJSON, err)
	}
	if err := Validate(projection); err != nil {
		return Projection{}, err
	}
	return projection, nil
}

// rejectDuplicateJSONKeys closes the gap between encoding/json's structural
// decoder and the contract's exact-object semantics. It walks only the
// supplied JSON bytes and does not retain or interpret any value.
func rejectDuplicateJSONKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := scanJSONValue(decoder); err != nil {
		return fmt.Errorf("aero-id profile projection JSON is invalid: %w", err)
	}
	return nil
}

func scanJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, isDelimiter := token.(json.Delim)
	if !isDelimiter {
		return nil
	}
	switch delimiter {
	case '{':
		seen := map[string]struct{}{}
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return errors.New("JSON object key is not a string")
			}
			if _, exists := seen[key]; exists {
				return fmt.Errorf("duplicate JSON object key %q", key)
			}
			seen[key] = struct{}{}
			if err := scanJSONValue(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	case '[':
		for decoder.More() {
			if err := scanJSONValue(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	default:
		return fmt.Errorf("unexpected JSON delimiter %q", delimiter)
	}
}

// Validate checks the versioned shape and deterministic membership ordering.
// It never interprets profile or membership values as authorization.
func Validate(projection Projection) error {
	if projection.SchemaVersion != SchemaVersion || projection.EvaluationMode != EvaluationMode ||
		projection.Source != Source || projection.Notice != Notice {
		return fmt.Errorf("aero-id profile projection envelope is invalid")
	}
	if !projection.OwnerDeclarationUnverified || !projection.ProfileAttributesUnverified ||
		!projection.MembershipAttributesUnverified || !zeroAuthority(projection.Authority) {
		return fmt.Errorf("aero-id profile projection claims authority")
	}
	if projection.Consistency != "eventual" && projection.Consistency != "bounded" && projection.Consistency != "strong" {
		return fmt.Errorf("aero-id profile projection consistency is invalid")
	}
	if err := validateOwner(projection.OwnerDeclaration); err != nil {
		return err
	}
	if err := validateProfile(projection.Profile); err != nil {
		return err
	}
	if len(projection.Memberships) > MaxMemberships {
		return fmt.Errorf("aero-id profile projection has too many memberships")
	}
	var previous string
	seen := make(map[string]struct{}, len(projection.Memberships))
	for index, membership := range projection.Memberships {
		if err := validateMembership(membership); err != nil {
			return fmt.Errorf("membership %d: %w", index, err)
		}
		key := membershipKey(membership)
		if _, exists := seen[key]; exists {
			return fmt.Errorf("aero-id profile projection has duplicate membership")
		}
		if index > 0 && previous >= key {
			return fmt.Errorf("aero-id profile projection memberships are not sorted")
		}
		seen[key] = struct{}{}
		previous = key
	}
	return nil
}

// BindOwner requires an exact owner tuple match.  Both values remain
// unverified declarations; this method only prevents confused-deputy display.
func (projection Projection) BindOwner(expected Owner) error {
	if err := validateOwner(expected); err != nil {
		return err
	}
	if projection.OwnerDeclaration != expected {
		return fmt.Errorf("aero-id profile projection owner does not match")
	}
	return nil
}

func validateOwner(owner Owner) error {
	for name, value := range map[string]string{
		"issuer": owner.Issuer, "subject": owner.Subject, "tenant_id": owner.TenantID,
	} {
		if !validText(value, 256, false) {
			return fmt.Errorf("aero-id profile owner %s is invalid", name)
		}
	}
	return nil
}

func validateProfile(profile Profile) error {
	if !validText(profile.AccountID, 128, false) || !validText(profile.DisplayName, 512, true) ||
		!validText(profile.AvatarURL, 2048, true) || !validText(profile.Locale, 128, true) ||
		!validText(profile.Timezone, 128, true) {
		return fmt.Errorf("aero-id profile fields are invalid")
	}
	return nil
}

func validateMembership(membership Membership) error {
	if !validText(membership.Source, 128, false) || !validText(membership.ScopeType, 128, false) ||
		!validText(membership.ScopeID, 256, false) || !validText(membership.Role, 128, false) ||
		!validText(membership.Status, 128, false) {
		return fmt.Errorf("aero-id membership fields are invalid")
	}
	return nil
}

func validText(value string, max int, allowEmpty bool) bool {
	if !utf8.ValidString(value) || len(value) > max || value != strings.TrimSpace(value) ||
		(!allowEmpty && value == "") || strings.ContainsFunc(value, unicode.IsControl) {
		return false
	}
	return true
}

func membershipKey(membership Membership) string {
	return strings.Join([]string{membership.Source, membership.ScopeType, membership.ScopeID, membership.Role, membership.Status}, "\x00")
}

func zeroAuthority(authority Authority) bool {
	return !authority.IdentityVerified && !authority.ProfileAuthoritative &&
		!authority.MembershipAuthoritative && !authority.AuthorizationGranted &&
		!authority.ExecutionAuthorized && !authority.ReservationCreated && !authority.DispatchPerformed
}
