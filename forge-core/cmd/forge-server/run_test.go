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
