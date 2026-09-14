package main

import (
	"bytes"
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/appserver"
)

func TestRunPrintsVersionWithoutStateDirectory(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := run(context.Background(), []string{"--version"}, &stdout, &stderr); code != 0 {
		t.Fatalf("exit = %d, stderr = %q", code, stderr.String())
	}
	if stdout.String() != "forge-server dev\n" {
		t.Fatalf("version output = %q", stdout.String())
	}
}

func TestRunRejectsInvalidConfigurationBeforeStateMutation(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := run(context.Background(), []string{"--listen", "0.0.0.0:7467"}, &stdout, &stderr)
	if code != 2 || !strings.Contains(stderr.String(), "state-dir") {
		t.Fatalf("exit = %d, stderr = %q", code, stderr.String())
	}
}

func TestRunRejectsPositionalArguments(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := run(context.Background(), []string{"unexpected"}, &stdout, &stderr)
	if code != 2 || !strings.Contains(stderr.String(), "positional") {
		t.Fatalf("exit = %d, stderr = %q", code, stderr.String())
	}
}

func TestRunRejectsMalformedOrDuplicateExecutionProfileBindings(t *testing.T) {
	valid := `{"project_id":"project-1","profile_id":"profile-1","profile_sha256":"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"}`
	for _, bindings := range [][]string{
		{`{"project_id":"project-1","profile_id":"profile-1"}`},
		{valid, valid},
	} {
		args := []string{"--execution-profile-binding", bindings[0]}
		if len(bindings) == 2 {
			args = append(args, "--execution-profile-binding", bindings[1])
		}
		var stdout, stderr bytes.Buffer
		if code := run(context.Background(), args, &stdout, &stderr); code != 2 ||
			!strings.Contains(stderr.String(), "execution profile policy") {
			t.Errorf("bindings=%#v exit=%d stderr=%q", bindings, code, stderr.String())
		}
	}
}

func TestAnnounceWritesOneVersionedJSONReceipt(t *testing.T) {
	var output bytes.Buffer
	ready := appserver.Ready{APIVersion: appserver.APIVersion, Event: "listening", Listen: "http://127.0.0.1:1"}
	if err := announce(&output)(ready); err != nil {
		t.Fatal(err)
	}
	want := "{\"api_version\":\"forgeos.app-server/v1\",\"event\":\"listening\",\"listen\":\"http://127.0.0.1:1\"}\n"
	if output.String() != want {
		t.Fatalf("ready receipt = %q", output.String())
	}
}
