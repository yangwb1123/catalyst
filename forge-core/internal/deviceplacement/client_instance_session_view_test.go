package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

func TestObserveClientInstanceSessionViewSortsAndBindsReadOnlyInstances(t *testing.T) {
	owner := validOwnerTuple()
	observation, err := ObserveClientInstanceSessionView(ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []ClientInstanceSessionViewInstance{
			{InstanceID: "client-web-1", ClientKind: ClientKindWeb, SessionIDs: []string{"conversation-2", "conversation-1"}, ObservedAtMS: 200500, Status: "idle"},
			{InstanceID: "client-cli-1", ClientKind: ClientKindCLI, SessionIDs: []string{"conversation-3"}, ObservedAtMS: 200400, Status: "active"},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil {
		t.Fatal(err)
	}
	if observation.SchemaVersion != ClientInstanceSessionViewSchemaVersion ||
		observation.EvaluationMode != ClientInstanceSessionViewEvaluationMode ||
		!observation.OwnerDeclarationUnverified || !observation.ReadOnly ||
		observation.Authority != (ClientInstanceSessionViewAuthority{}) {
		t.Fatalf("unexpected view envelope: %#v", observation)
	}
	if len(observation.Instances) != 2 || observation.Instances[0].InstanceID != "client-cli-1" ||
		observation.Instances[1].InstanceID != "client-web-1" ||
		!bytes.Equal(mustJSON(t, observation.Instances[1].SessionIDs), []byte(`["conversation-1","conversation-2"]`)) {
		t.Fatalf("view ordering = %#v", observation.Instances)
	}
}

func TestClientInstanceSessionViewRejectsConfusedDeclarations(t *testing.T) {
	owner := validOwnerTuple()
	base := ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []ClientInstanceSessionViewInstance{{
			InstanceID: "client-cli-1", ClientKind: ClientKindCLI,
			SessionIDs: []string{"conversation-1"}, ObservedAtMS: 200500, Status: "active",
		}},
	}
	tests := []struct {
		name   string
		change func(*ClientInstanceSessionViewRequest)
	}{
		{"duplicate instance", func(value *ClientInstanceSessionViewRequest) {
			value.Instances = append(value.Instances, value.Instances[0])
		}},
		{"duplicate session", func(value *ClientInstanceSessionViewRequest) {
			value.Instances[0].SessionIDs = []string{"conversation-1", "conversation-1"}
		}},
		{"unknown client kind", func(value *ClientInstanceSessionViewRequest) {
			value.Instances[0].ClientKind = "agent-hub"
		}},
		{"unknown status", func(value *ClientInstanceSessionViewRequest) {
			value.Instances[0].Status = "running"
		}},
		{"missing observation time", func(value *ClientInstanceSessionViewRequest) {
			value.Instances[0].ObservedAtMS = 0
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			input := base
			input.Instances = append([]ClientInstanceSessionViewInstance(nil), base.Instances...)
			input.Instances[0].SessionIDs = append([]string(nil), base.Instances[0].SessionIDs...)
			test.change(&input)
			if _, err := ObserveClientInstanceSessionView(input); err == nil {
				t.Fatal("invalid client-instance/session declaration accepted")
			}
		})
	}
}

func TestDecodeClientInstanceSessionViewFixtureAndRejectsMutations(t *testing.T) {
	fixturePath := "../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
	encoded, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	observation, err := DecodeClientInstanceSessionView(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	if len(observation.Instances) != 5 || observation.Instances[0].InstanceID != "client-app-001" ||
		observation.Instances[0].ClientKind != ClientKindApp ||
		observation.Instances[2].ClientKind != ClientKindMobile ||
		observation.Instances[3].ClientKind != ClientKindTUI ||
		observation.Instances[4].ClientKind != ClientKindWeb {
		t.Fatalf("fixture instances = %#v", observation.Instances)
	}

	var value map[string]any
	if err := json.Unmarshal(encoded, &value); err != nil {
		t.Fatal(err)
	}
	value["authority"].(map[string]any)["prompt_write_authorized"] = true
	authority, _ := json.Marshal(value)
	if _, err := DecodeClientInstanceSessionView(bytes.NewReader(authority)); err == nil {
		t.Fatal("authority mutation accepted")
	}

	value["authority"].(map[string]any)["prompt_write_authorized"] = false
	value["unexpected"] = true
	unknown, _ := json.Marshal(value)
	if _, err := DecodeClientInstanceSessionView(bytes.NewReader(unknown)); err == nil {
		t.Fatal("unknown field accepted")
	}

	duplicate := []byte(`{"schema_version":"forge.client-instance-session-view/v1","schema_version":"forge.client-instance-session-view/v1"}`)
	if _, err := DecodeClientInstanceSessionView(bytes.NewReader(duplicate)); err == nil {
		t.Fatal("duplicate field accepted")
	}
}

func mustJSON(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}
