package promptappendreceipt

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"reflect"
	"strings"
	"testing"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestPromptAppendReceiptCanonicalFixture(t *testing.T) {
	path := os.Getenv("FORGE_PROMPT_APPEND_RECEIPT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_PROMPT_APPEND_RECEIPT_FIXTURE is set by the contract script")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	value, err := decode(raw)
	if err != nil {
		t.Fatal(err)
	}
	if err := value.Validate(); err != nil {
		t.Fatal(err)
	}
	projected, err := Project(model.Owner{
		Issuer: value.Owner.Issuer, Subject: value.Owner.Subject, TenantID: value.Owner.TenantID,
	}, value.Request.ConversationID, "send this from another client", "prompt-key-003",
		value.Request.ExpectedVersion, model.ConversationPrompt{
			ID: value.Receipt.PromptID, ConversationID: value.Receipt.ConversationID,
			Role: value.Receipt.Role, Content: "send this from another client",
			CreatedAtMS: value.Receipt.CreatedAtMS,
		}, value.Receipt.Replayed)
	if err != nil || !reflect.DeepEqual(projected, value) {
		t.Fatalf("projection=%#v fixture=%#v err=%v", projected, value, err)
	}
}

func TestPromptAppendReceiptRejectsInvalidInput(t *testing.T) {
	owner := model.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	prompt := model.ConversationPrompt{
		ID: "prompt-003", ConversationID: "conversation-001", Role: "user",
		Content: "send this from another client", CreatedAtMS: 300,
	}
	for name, input := range map[string]struct{ content, key string }{
		"empty content":             {content: "", key: "prompt-key-003"},
		"oversized content":         {content: strings.Repeat("x", maxPromptContentBytes+1), key: "prompt-key-003"},
		"empty idempotency key":     {content: prompt.Content, key: ""},
		"oversized idempotency key": {content: prompt.Content, key: strings.Repeat("k", maxIdempotencyKeyBytes+1)},
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := Project(owner, prompt.ConversationID, input.content, input.key, 2, prompt, false); err == nil {
				t.Fatal("invalid Prompt append input accepted")
			}
		})
	}
}

func TestPromptAppendReceiptRejectsWireAndBindingDrift(t *testing.T) {
	raw := canonicalFixture(t)
	mutations := map[string][]byte{
		"unknown":   append(bytes.TrimSuffix(bytes.TrimSpace(raw), []byte("}")), []byte(`,"unexpected":true}`)...),
		"duplicate": append(bytes.TrimSuffix(bytes.TrimSpace(raw), []byte("}")), []byte(`,"schema_version":"forge.prompt-append-receipt/v1"}`)...),
		"trailing":  append(bytes.TrimSpace(raw), []byte(" {}")...),
		"binding":   bytes.Replace(raw, []byte(`"conversation_id": "conversation-001"`), []byte(`"conversation_id": "conversation-foreign"`), 1),
		"authority": bytes.Replace(raw, []byte(`"audit_published": false`), []byte(`"audit_published": true`), 1),
	}
	for name, mutation := range mutations {
		t.Run(name, func(t *testing.T) {
			value, err := decode(mutation)
			if err == nil && value.Validate() == nil {
				t.Fatalf("mutation %q was accepted: %#v", name, value)
			}
		})
	}
}

func canonicalFixture(t *testing.T) []byte {
	t.Helper()
	path := os.Getenv("FORGE_PROMPT_APPEND_RECEIPT_FIXTURE")
	if path == "" {
		t.Fatal("FORGE_PROMPT_APPEND_RECEIPT_FIXTURE is required")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func decode(raw []byte) (Envelope, error) {
	if err := rejectDuplicateKeys(raw); err != nil {
		return Envelope{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value Envelope
	if err := decoder.Decode(&value); err != nil {
		return Envelope{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return Envelope{}, io.ErrUnexpectedEOF
		}
		return Envelope{}, err
	}
	return value, nil
}

func rejectDuplicateKeys(raw []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	return scanJSON(decoder)
}

func scanJSON(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	switch token {
	case json.Delim('{'):
		seen := map[string]bool{}
		for decoder.More() {
			token, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := token.(string)
			if !ok || seen[key] {
				return io.ErrUnexpectedEOF
			}
			seen[key] = true
			if err := scanJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	case json.Delim('['):
		for decoder.More() {
			if err := scanJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	}
	return nil
}
