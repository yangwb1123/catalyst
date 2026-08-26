package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadTraceEventsFiltersRunIDAndReportsMalformed(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	body := `{"seq":1,"kind":"agent","name":"planner","status":"ok","run_id":"run-alpha"}
{not-json}
{"seq":2,"kind":"agent","name":"implementer","status":"ok","run_id":"run-beta"}
`
	if err := os.WriteFile(filepath.Join(dir, "trace.jsonl"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	events, malformed, err := loadTraceEvents(root, "agent", "ok", "", "run-a")
	if err != nil {
		t.Fatal(err)
	}
	if malformed != 1 {
		t.Fatalf("malformed = %d, want 1", malformed)
	}
	if len(events) != 1 || events[0].RunID != "run-alpha" {
		t.Fatalf("events = %+v, want only run-alpha", events)
	}
}

func TestLoadTraceEventsMissingFile(t *testing.T) {
	events, malformed, err := loadTraceEvents(t.TempDir(), "", "", "", "")
	if err == nil {
		t.Fatal("missing trace must return an actionable error")
	}
	if events != nil || malformed != 0 {
		t.Fatalf("missing trace = events=%v malformed=%d", events, malformed)
	}
}

func TestLoadTraceEventsCountsUnsupportedFormatAsMalformed(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	body := `{"_format":"forgeos.trace.v2","seq":1,"kind":"agent","status":"ok"}
{"_format":"forgeos.trace.v1","seq":2,"kind":"gate","status":"PASS"}
`
	if err := os.WriteFile(filepath.Join(dir, "trace.jsonl"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	events, malformed, err := loadTraceEvents(root, "", "", "", "")
	if err != nil {
		t.Fatal(err)
	}
	if malformed != 1 || len(events) != 1 || events[0].Seq != 2 {
		t.Fatalf("events=%+v malformed=%d", events, malformed)
	}
}

func TestLoadTraceEventsFiltersCanonicalRuntimeKinds(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	body := `{"_format":"forgeos.trace.v1","seq":1,"kind":"decision","status":"ok"}
{"_format":"forgeos.trace.v1","seq":2,"kind":"overload_backoff","status":"retry"}
{"_format":"forgeos.trace.v1","seq":3,"kind":"stale_increment","status":"stale"}
{"_format":"forgeos.trace.v1","seq":4,"kind":"error","status":"failed"}
`
	if err := os.WriteFile(filepath.Join(dir, "trace.jsonl"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	for seq, kind := range []string{"decision", "overload_backoff", "stale_increment", "error"} {
		events, malformed, err := loadTraceEvents(root, kind, "", "", "")
		if err != nil || malformed != 0 || len(events) != 1 || events[0].Seq != seq+1 {
			t.Errorf("kind=%s events=%+v malformed=%d err=%v", kind, events, malformed, err)
		}
	}
}
