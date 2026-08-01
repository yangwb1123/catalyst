package graphrelease

import "forgeos/forge-core/internal/graphdispatch"

func validateContractBindings(control ReleaseControl, facts journalFacts) error {
	contractJSON, err := graphdispatch.MarshalContract(control.Contract)
	if err != nil || len(contractJSON) == 0 ||
		len(contractJSON) > graphdispatch.MaxControlSnapshotBytes ||
		graphdispatch.ValidateContractSource(control.Contract, control.Manifest, control.Plan) != nil {
		return errInvalidControl
	}
	if validateOriginalControl(control, facts) != nil ||
		validateContractHeaderBindings(control, facts) != nil ||
		validateContractRecord(control, facts, contractJSON) != nil ||
		validateContractEvent(control, facts, contractJSON) != nil {
		return errInvalidControl
	}
	return nil
}

func validateOriginalControl(control ReleaseControl, facts journalFacts) error {
	snapshot := graphdispatch.ControlSnapshot{
		V:                        graphdispatch.ControlSnapshotVersion,
		SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		GraphRunVersion:          1, GraphRunID: control.GraphRun.GraphRunID,
		GraphID:              control.GraphRun.GraphID,
		SourceSnapshotSHA256: control.GraphRun.SourceSnapshotSHA256,
		GraphManifestSHA256:  control.GraphRun.GraphManifestSHA256,
		CorePlanSHA256:       control.GraphRun.PlanSHA256,
		LastEventSeq:         1, LastEventSHA256: facts.PreparedSHA256,
		ExecutionContractPresent: false, DispatchAuthorityReleased: false,
		Plan: control.Plan, Manifest: control.Manifest,
		SnapshotSHA256: control.Contract.ControlSnapshotSHA256,
	}
	if graphdispatch.ValidateControlSnapshot(snapshot) != nil {
		return errInvalidControl
	}
	return nil
}

func validateContractHeaderBindings(control ReleaseControl, facts journalFacts) error {
	contract := control.Contract
	run := control.GraphRun
	valid := contract.GraphRunID == run.GraphRunID && contract.GraphID == run.GraphID &&
		contract.SourceSnapshotSHA256 == run.SourceSnapshotSHA256 &&
		contract.GraphManifestSHA256 == run.GraphManifestSHA256 &&
		contract.CorePlanSHA256 == run.PlanSHA256 &&
		contract.ExpectedLastEventSeq == 1 &&
		contract.ExpectedLastEventSHA256 == facts.PreparedSHA256 &&
		contract.ExecutionContractPresent && !contract.DispatchAuthorityReleased
	if !valid {
		return errInvalidControl
	}
	return nil
}

func validateContractRecord(control ReleaseControl, facts journalFacts, contractJSON []byte) error {
	record := control.ContractRecord
	contract := control.Contract
	valid := record.V == 1 && record.ContractID == contract.ContractID &&
		record.GraphRunID == contract.GraphRunID && record.NodeID == contract.Node.NodeID &&
		record.Attempt == 1 && record.Attempt == contract.Node.Attempt &&
		record.ControlSnapshotSHA256 == contract.ControlSnapshotSHA256 &&
		record.ContractSHA256 == contract.ContractSHA256 &&
		record.ContractBytes == uint64(len(contractJSON)) &&
		record.RequestSHA256 == contract.Request.RequestSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.ExpectedLastEventSeq == 1 &&
		record.ExpectedLastEventSHA256 == facts.PreparedSHA256 &&
		validSignedTime(record.CreatedAtMS)
	if !valid {
		return errInvalidControl
	}
	return nil
}

func validateContractEvent(control ReleaseControl, facts journalFacts, contractJSON []byte) error {
	event := facts.Contract
	record := control.ContractRecord
	valid := event.ControlSnapshotSHA256 == record.ControlSnapshotSHA256 &&
		event.ContractID == record.ContractID && event.ContractSHA256 == record.ContractSHA256 &&
		event.ContractBytes == uint64(len(contractJSON)) && event.NodeID == record.NodeID &&
		event.Attempt == 1 && event.Attempt == record.Attempt &&
		event.RequestSHA256 == record.RequestSHA256 &&
		event.ProjectLaneSHA256 == record.ProjectLaneSHA256 &&
		event.AdmittedAtMS == record.CreatedAtMS
	if !valid {
		return errInvalidControl
	}
	return nil
}
