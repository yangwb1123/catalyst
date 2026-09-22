package aeroidprofile

import (
	"encoding/json"
	"os"
	"testing"
)

func TestProjectionFixture(t *testing.T) {
	path := os.Getenv("FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE")
	if path == "" {
		t.Skip("FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	projection, err := Decode(data)
	if err != nil {
		t.Fatal(err)
	}
	if err := projection.BindOwner(projection.OwnerDeclaration); err != nil {
		t.Fatalf("same owner must bind: %v", err)
	}
	foreign := projection.OwnerDeclaration
	foreign.TenantID = "tenant-foreign"
	if err := projection.BindOwner(foreign); err == nil {
		t.Fatal("foreign owner unexpectedly bound")
	}
}

func TestProjectionRejectsAuthorityUnknownAndConfusedMemberships(t *testing.T) {
	path := os.Getenv("FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE")
	if path == "" {
		t.Skip("FORGE_AERO_ID_PROFILE_PROJECTION_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutate := func(name string, fn func(map[string]any)) {
		t.Run(name, func(t *testing.T) {
			var object map[string]any
			if err := json.Unmarshal(data, &object); err != nil {
				t.Fatal(err)
			}
			fn(object)
			mutated, err := json.Marshal(object)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := Decode(mutated); err == nil {
				t.Fatal("mutated projection unexpectedly accepted")
			}
		})
	}
	mutate("authority", func(object map[string]any) {
		object["authority"].(map[string]any)["authorization_granted"] = true
	})
	mutate("unknown", func(object map[string]any) { object["unexpected"] = true })
	mutate("unsorted_memberships", func(object map[string]any) {
		memberships := object["memberships"].([]any)
		if len(memberships) > 1 {
			memberships[0], memberships[1] = memberships[1], memberships[0]
		}
	})
	if _, err := Decode(append(data, []byte(`{"schema_version":"forge.aero-id-profile-projection/v1"}`)...)); err == nil {
		t.Fatal("trailing JSON unexpectedly accepted")
	}
	duplicate := append(append([]byte{}, data[:len(data)-2]...), []byte(`,"source":"aero-id"}`)...)
	if _, err := Decode(duplicate); err == nil {
		t.Fatal("duplicate JSON key unexpectedly accepted")
	}
}
