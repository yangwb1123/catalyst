package trace

import (
	"bytes"
	"strings"
	"testing"
)

func TestEmitRedactsCredentialValuesFromDetail(t *testing.T) {
	assignment := strings.Join([]string{"API", "_KEY=example-private-value"}, "")
	bearer := strings.Join([]string{"Bearer ", strings.Repeat("a", 24)}, "")
	prefixed := "sk-" + strings.Repeat("x", 16)
	var out bytes.Buffer
	tracer := NewTracer(&out)
	if err := tracer.Emit(Event{Kind: "error", Name: "agent", Detail: assignment + " " + bearer + " " + prefixed}); err != nil {
		t.Fatal(err)
	}
	line := out.String()
	for _, leaked := range []string{"example-private-value", strings.Repeat("a", 24), prefixed} {
		if strings.Contains(line, leaked) {
			t.Fatalf("trace leaked credential-shaped value %q: %s", leaked, line)
		}
	}
	if got := strings.Count(line, "[REDACTED]"); got != 3 {
		t.Fatalf("redaction count = %d, want 3: %s", got, line)
	}
}

func TestEmitRedactsVendorQualifiedCredentialAssignments(t *testing.T) {
	assignments := []struct {
		key, value string
	}{
		{"ANTHROPIC_API_KEY", "anthropic-private-value"},
		{"GITHUB_TOKEN", "github-private-value"},
		{"AWS_SECRET_ACCESS_KEY", "aws-private-value"},
		{"MY_SERVICE_AUTH_TOKEN", "service-private-value"},
	}
	var detail strings.Builder
	for _, assignment := range assignments {
		if detail.Len() > 0 {
			detail.WriteByte(' ')
		}
		detail.WriteString(assignment.key + "=" + assignment.value)
	}
	var out bytes.Buffer
	tracer := NewTracer(&out)
	if err := tracer.Emit(Event{Kind: "error", Name: "agent", Detail: detail.String()}); err != nil {
		t.Fatal(err)
	}
	line := out.String()
	for _, assignment := range assignments {
		if strings.Contains(line, assignment.value) {
			t.Errorf("trace leaked %s value: %s", assignment.key, line)
		}
		if !strings.Contains(line, assignment.key+"=[REDACTED]") {
			t.Errorf("trace did not preserve/redact %s assignment: %s", assignment.key, line)
		}
	}
}

func TestEmitLeavesOrdinaryDetailUnchanged(t *testing.T) {
	const detail = "gates_green=true roadmap=80%"
	var out bytes.Buffer
	tracer := NewTracer(&out)
	if err := tracer.Emit(Event{Kind: "converge", Name: "build", Detail: detail}); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), detail) {
		t.Fatalf("ordinary trace detail changed: %s", out.String())
	}
}
