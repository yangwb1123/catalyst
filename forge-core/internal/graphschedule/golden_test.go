package graphschedule

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"testing"
)

func TestSharedExecutionScheduleGolden(t *testing.T) {
	fixture := readScheduleFixture(t)
	if fixture.ControlFixture != "group-agent-node-execution-contract-v1.json" {
		t.Fatalf("unknown control fixture %q", fixture.ControlFixture)
	}
	value := mustBuildSchedule(t)
	payload, payloadErr := canonicalBytes(schedulePayloadFrom(value))
	encoded, marshalErr := MarshalSchedule(value)
	if payloadErr != nil || string(payload) != fixture.CanonicalSchedulePayloadJSON {
		t.Fatalf("schedule payload differs: err=%v\n%s", payloadErr, payload)
	}
	if marshalErr != nil || string(encoded) != fixture.CanonicalExecutionScheduleJSON {
		t.Fatalf("schedule differs: err=%v\n%s", marshalErr, encoded)
	}
	if value.ScheduleSHA256 != fixture.ScheduleSHA256 || value.ScheduleID != fixture.ScheduleID {
		t.Fatalf("schedule identity = %s / %s", value.ScheduleID, value.ScheduleSHA256)
	}
	if strings.HasSuffix(string(encoded), "\n") || strings.Contains(string(encoded), `\u003c`) {
		t.Fatal("schedule is not exact non-HTML-escaped canonical JSON")
	}
}

func TestEmitSharedExecutionScheduleGolden(t *testing.T) {
	if os.Getenv("FORGE_EMIT_EXECUTION_SCHEDULE_FIXTURE") != "1" {
		t.Skip("fixture emission is opt-in")
	}
	value := mustBuildSchedule(t)
	payload, err := canonicalBytes(schedulePayloadFrom(value))
	if err != nil {
		t.Fatalf("encode schedule payload: %v", err)
	}
	encoded, err := MarshalSchedule(value)
	if err != nil {
		t.Fatalf("encode schedule: %v", err)
	}
	fixture := sharedScheduleFixture{
		V: 1, ControlFixture: "group-agent-node-execution-contract-v1.json",
		CanonicalSchedulePayloadJSON: string(payload), ScheduleSHA256: value.ScheduleSHA256,
		ScheduleID: value.ScheduleID, CanonicalExecutionScheduleJSON: string(encoded),
	}
	indented, err := json.MarshalIndent(fixture, "", "  ")
	if err != nil {
		t.Fatalf("encode shared fixture: %v", err)
	}
	fmt.Printf("FIXTURE_BEGIN\n%s\nFIXTURE_END\n", indented)
}
