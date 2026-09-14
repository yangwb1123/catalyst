package auditprojection

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"slices"
	"testing"
)

func TestPromptAcceptedProjectionMatchesGoldenContract(t *testing.T) {
	event, err := Project(projectionOwner(), projectionChange())
	if err != nil {
		t.Fatal(err)
	}
	actual, err := json.MarshalIndent(event, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	actual = append(actual, '\n')
	fixturePath := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-prompt-accepted-audit-v1.json")
	expected, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(actual, expected) {
		t.Fatalf("projection differs from %s\nactual:\n%s", fixturePath, actual)
	}
}

func TestPromptAcceptedSchemaIsClosedAndMatchesTheProjection(t *testing.T) {
	path := filepath.Join("..", "..", "..", "docs", "contracts", "forge-prompt-accepted-audit-v1.schema.json")
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	type schemaNode struct {
		AdditionalProperties *bool                 `json:"additionalProperties"`
		Required             []string              `json:"required"`
		Properties           map[string]schemaNode `json:"properties"`
		MaxLength            int                   `json:"maxLength"`
		Pattern              string                `json:"pattern"`
		Const                any                   `json:"const"`
	}
	var schema schemaNode
	if err := json.Unmarshal(encoded, &schema); err != nil {
		t.Fatal(err)
	}
	want := []string{
		"action", "actor", "aggregate_id", "aggregate_type", "aggregate_version", "data_classification",
		"event_id", "event_type", "idempotency_key", "occurred_at", "operation_id", "outcome", "payload",
		"retention_class", "schema_id", "schema_version", "source_system", "tenant_id",
	}
	if schema.AdditionalProperties == nil || *schema.AdditionalProperties || len(schema.Properties) != len(want) {
		t.Fatalf("schema is not closed or field count changed: %#v", schema)
	}
	for name := range schema.Properties {
		if !slices.Contains(want, name) {
			t.Errorf("schema unexpectedly permits field %q", name)
		}
	}
	if len(schema.Required) != len(want) {
		t.Fatalf("required fields = %v", schema.Required)
	}
	for _, name := range want {
		if !slices.Contains(schema.Required, name) {
			t.Errorf("schema field %q is not required", name)
		}
	}
	for _, name := range []string{"event_id", "idempotency_key"} {
		if schema.Properties[name].Pattern != "^[0-9a-f]{64}$" {
			t.Errorf("schema field %q does not require a deterministic SHA-256 identifier", name)
		}
	}
	for name, wantValue := range map[string]any{
		"source_system": "forge-runtime", "event_type": EventType, "schema_id": SchemaID,
		"schema_version": float64(1), "data_classification": "internal", "retention_class": "standard",
	} {
		if schema.Properties[name].Const != wantValue {
			t.Errorf("schema field %q const = %#v, want %#v", name, schema.Properties[name].Const, wantValue)
		}
	}
	for _, name := range []string{"tenant_id", "operation_id", "aggregate_id"} {
		field := schema.Properties[name]
		if field.MaxLength != maxAuditKeyBytes || field.Pattern != `^[^\s:/\\]+$` {
			t.Errorf("schema field %q does not match Audit Governance key constraints: %#v", name, field)
		}
	}
	actor := schema.Properties["actor"]
	if actor.AdditionalProperties == nil || *actor.AdditionalProperties ||
		len(actor.Properties) != 2 || !slices.Contains(actor.Required, "id") || !slices.Contains(actor.Required, "type") ||
		actor.Properties["id"].Pattern != "^[0-9a-f]{64}$" || actor.Properties["type"].Const != "user" {
		t.Errorf("actor schema does not enforce the pseudonym contract: %#v", actor)
	}
	payload := schema.Properties["payload"]
	if payload.AdditionalProperties == nil || *payload.AdditionalProperties ||
		len(payload.Properties) != 1 || len(payload.Required) != 1 || payload.Required[0] != "content_included" ||
		payload.Properties["content_included"].Const != false {
		t.Errorf("payload schema permits content beyond its exclusion marker: %#v", payload)
	}
}
