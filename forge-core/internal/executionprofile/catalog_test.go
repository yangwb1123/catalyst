package executionprofile

import (
	"context"
	"errors"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"strings"
	"testing"

	"forgeos/forge-core/internal/runtimebridge"
)

type fakeProjectConversationReader struct {
	identity model.OwnedProjectConversationIdentity
	err      error
	calls    int
	owner    model.Owner
	convID   string
}

func (reader *fakeProjectConversationReader) OwnedProjectConversationIdentity(
	_ context.Context,
	owner model.Owner,
	conversationID string,
) (model.OwnedProjectConversationIdentity, error) {
	reader.calls++
	reader.owner = owner
	reader.convID = conversationID
	return reader.identity, reader.err
}

func TestResolveConversationProfileUsesHubProjectAndExactOwner(t *testing.T) {
	profile := intentmodel.ServerExecutionProfile{ID: "profile-build-v1", SHA256: [32]byte{1, 2, 3}}
	catalog, err := New([]Binding{
		{ProjectID: "project-allowed", Profile: profile},
		{ProjectID: "project-other", Profile: intentmodel.ServerExecutionProfile{ID: "profile-other", SHA256: [32]byte{4, 5, 6}}},
	})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://id.example", Subject: "account-1", TenantID: "tenant-1"}
	reader := &fakeProjectConversationReader{identity: model.OwnedProjectConversationIdentity{
		ConversationID: "conversation-1", ProjectID: "project-allowed",
	}}
	got, err := catalog.ResolveConversationProfile(context.Background(), reader, owner, "conversation-1")
	if err != nil || got != profile || reader.calls != 1 || reader.owner != owner || reader.convID != "conversation-1" {
		t.Fatalf("resolved profile=%#v reader=%#v error=%v", got, reader, err)
	}
}

func TestResolveConversationProfileFailsClosedWithoutHubProjectBinding(t *testing.T) {
	profile := intentmodel.ServerExecutionProfile{ID: "profile-build-v1", SHA256: [32]byte{1}}
	catalog, err := New([]Binding{{ProjectID: "project-allowed", Profile: profile}})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://id.example", Subject: "account-1", TenantID: "tenant-1"}
	reader := &fakeProjectConversationReader{identity: model.OwnedProjectConversationIdentity{
		ConversationID: "conversation-1", ProjectID: "project-caller-supplied",
	}}
	if _, err := catalog.ResolveConversationProfile(context.Background(), reader, owner, "conversation-1"); !errors.Is(err, ErrProfileUnavailable) {
		t.Fatalf("unconfigured Hub Project error=%v", err)
	}
	reader.identity.ConversationID = "another-conversation"
	reader.identity.ProjectID = "project-allowed"
	if _, err := catalog.ResolveConversationProfile(context.Background(), reader, owner, "conversation-1"); !errors.Is(err, ErrInvalidProjectContext) {
		t.Fatalf("mismatched Hub identity error=%v", err)
	}
	reader.err = &runtimebridge.Error{Code: "not_found"}
	if _, err := catalog.ResolveConversationProfile(context.Background(), reader, owner, "conversation-1"); !errors.As(err, new(*runtimebridge.Error)) {
		t.Fatalf("owner-filtered Hub error was not preserved: %v", err)
	}
}

func TestCatalogConfigurationRejectsDuplicatesAndMalformedBindings(t *testing.T) {
	valid := Binding{ProjectID: "project-1", Profile: intentmodel.ServerExecutionProfile{ID: "profile-1"}}
	for _, bindings := range [][]Binding{
		{valid, valid},
		{{ProjectID: " ", Profile: valid.Profile}},
		{{ProjectID: "project-1", Profile: intentmodel.ServerExecutionProfile{}}},
		make([]Binding, maxBindings+1),
	} {
		if _, err := New(bindings); !errors.Is(err, ErrInvalidPolicy) {
			t.Errorf("New(%#v) error=%v, want invalid policy", bindings, err)
		}
	}
	if _, err := New(nil); err != nil {
		t.Fatalf("empty catalog should be a disabled fail-closed policy: %v", err)
	}
}

func TestParseBindingConfigRequiresExactCanonicalServerPolicy(t *testing.T) {
	valid := `{"project_id":"project-1","profile_id":"profile-1","profile_sha256":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"}`
	binding, err := ParseBindingConfig(valid)
	if err != nil || binding.ProjectID != "project-1" || binding.Profile.ID != "profile-1" ||
		binding.Profile.SHA256 != [32]byte{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
			16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31} {
		t.Fatalf("parsed binding=%#v error=%v", binding, err)
	}
	for _, invalid := range []string{
		strings.Replace(valid, `"profile_id":"profile-1"`, `"profile_id":"profile-1","profile_id":"profile-2"`, 1),
		strings.Replace(valid, `,"profile_sha256":`, `,"extra":1,"profile_sha256":`, 1),
		strings.Replace(valid, `"profile_sha256":"000102`, `"profile_sha256":"000102`, 1)[:len(valid)-1],
		strings.Replace(valid, `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`,
			`000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1E`, 1),
		"null",
	} {
		if _, err := ParseBindingConfig(invalid); !errors.Is(err, ErrInvalidPolicy) {
			t.Errorf("ParseBindingConfig(%q) error=%v", invalid, err)
		}
	}
}

func TestEmptyCatalogFailsBeforeHubLookup(t *testing.T) {
	catalog, err := New(nil)
	if err != nil {
		t.Fatal(err)
	}
	reader := &fakeProjectConversationReader{}
	_, err = catalog.ResolveConversationProfile(context.Background(), reader,
		model.Owner{Issuer: "https://id.example", Subject: "a", TenantID: "t"}, "conversation-1")
	if !errors.Is(err, ErrProfileUnavailable) || reader.calls != 0 {
		t.Fatalf("empty catalog error=%v calls=%d", err, reader.calls)
	}
}
