package auditprojection

import (
	"errors"
	"strings"
	"testing"

	"forgeos/forge-core/internal/runtimebridge/model"
)

func TestProjectBindsEventIdentityToOwnerAndCommittedChange(t *testing.T) {
	owner := projectionOwner()
	change := projectionChange()
	first, err := Project(owner, change)
	if err != nil {
		t.Fatal(err)
	}
	if first.Actor.ID == owner.Subject || len(first.Actor.ID) != 64 || first.Actor.Type != "user" {
		t.Fatalf("actor identity is not issuer-qualified and pseudonymized: %#v", first.Actor)
	}
	replayed, err := Project(owner, change)
	if err != nil || replayed.EventID != first.EventID || replayed.IdempotencyKey != first.IdempotencyKey {
		t.Fatalf("repeat projection = %#v, %v", replayed, err)
	}

	otherOwner := owner
	otherOwner.Subject = "subject-b"
	other, err := Project(otherOwner, change)
	if err != nil || other.EventID == first.EventID {
		t.Fatalf("different verified subject reused identity: %#v, %v", other, err)
	}
	if other.Actor.ID == first.Actor.ID {
		t.Fatalf("different verified subject reused actor pseudonym: %#v", other.Actor)
	}
	otherIssuer := owner
	otherIssuer.Issuer = "https://other-id.example"
	other, err = Project(otherIssuer, projectionChange())
	if err != nil || other.Actor.ID == first.Actor.ID || other.EventID == first.EventID {
		t.Fatalf("different issuer reused owner-bound identity: %#v, %v", other, err)
	}
	change.Cursor++
	other, err = Project(owner, change)
	if err != nil || other.EventID == first.EventID {
		t.Fatalf("different committed change reused identity: %#v, %v", other, err)
	}
}

func TestProjectRejectsInvalidOrUnsupportedMetadata(t *testing.T) {
	tests := []struct {
		name   string
		owner  model.Owner
		change model.Change
	}{
		{name: "missing tenant", owner: model.Owner{Issuer: "https://id.example", Subject: "sub-123"}, change: projectionChange()},
		{name: "control character subject", owner: model.Owner{Issuer: "https://id.example", Subject: "sub\n123", TenantID: "tenant-a"}, change: projectionChange()},
		{name: "tenant stream delimiter", owner: model.Owner{Issuer: "https://id.example", Subject: "sub-123", TenantID: "tenant:a"}, change: projectionChange()},
		{name: "tenant archive key overflow", owner: model.Owner{Issuer: "https://id.example", Subject: "sub-123", TenantID: strings.Repeat("t", 86)}, change: projectionChange()},
		{name: "conversation creation", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.Kind = "conversation_created" })},
		{name: "unsupported journal schema", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.SchemaVersion = 2 })},
		{name: "missing prompt identity", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.EntityID = "" })},
		{name: "prompt archive path separator", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.EntityID = "prompt/19" })},
		{name: "conversation stream delimiter", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.ConversationID = "conversation:7" })},
		{name: "prompt archive key overflow", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.EntityID = strings.Repeat("p", 86) })},
		{name: "timestamp outside audit range", owner: projectionOwner(), change: changeWith(func(change *model.Change) { change.CreatedAtMS = ^uint64(0) })},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := Project(test.owner, test.change)
			if !errors.Is(err, ErrInvalidProjection) {
				t.Fatalf("Project error = %v", err)
			}
		})
	}
}

func projectionOwner() model.Owner {
	return model.Owner{Issuer: "https://id.example", Subject: "sub-123", TenantID: "tenant-a"}
}

func projectionChange() model.Change {
	return model.Change{
		Cursor: 17, SchemaVersion: 1, ConversationID: "conversation-7", EntityID: "prompt-19",
		AggregateVersion: 4, Kind: "prompt_appended", CreatedAtMS: 1789300800000,
	}
}

func changeWith(update func(*model.Change)) model.Change {
	change := projectionChange()
	update(&change)
	return change
}
