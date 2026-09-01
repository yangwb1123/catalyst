package application

import (
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const testUnixMS = int64(1788091200000)

func testMeta(serial int) CommandMeta {
	return CommandMeta{
		ActorRef:  core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		CommandID: testID("cmd", serial), CorrelationID: testID("cor", serial),
		ExpectedVersion: 0, IdempotencyKey: fmt.Sprintf("workspace-command-%04d", serial),
		IssuedAtUnixMS: testUnixMS, MessageID: testID("msg", serial),
	}
}

func testSpaceRequest(serial int) CreateSpaceRequest {
	return CreateSpaceRequest{
		Meta: testMeta(serial), Name: fmt.Sprintf("Space %d", serial),
		SpaceID: testID("spc", serial),
	}
}

func testProjectRequest(serial, spaceSerial int) RegisterProjectRequest {
	return RegisterProjectRequest{
		Meta: testMeta(serial), Alias: fmt.Sprintf("project-%d", serial),
		ProjectID: testID("prj", serial), RootPath: fmt.Sprintf("/workspace/project-%d", serial),
		SpaceID: testID("spc", spaceSerial),
	}
}

func testSnapshotRequest(serial, projectSerial, spaceSerial int) RecordProjectSnapshotRequest {
	return RecordProjectSnapshotRequest{
		Meta: testMeta(serial), CapturedAtUnixMS: testUnixMS,
		ObservationRef: core.RecordRef{
			RecordID:     fmt.Sprintf("snapshot-observation-%d", serial),
			RecordSHA256: fmt.Sprintf("%064x", serial),
			RecordType:   "forge.observation.project_snapshot",
		},
		ProjectID: testID("prj", projectSerial), ProjectSnapshotID: testID("psn", serial),
		SpaceID: testID("spc", spaceSerial),
	}
}

func testService(journal Journal) *Service {
	serial := 0
	service, err := newService(journal, deterministicIdentity(&serial), func() int64 {
		return testUnixMS + 1
	})
	if err != nil {
		panic(err)
	}
	return service
}
