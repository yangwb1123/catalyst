package graphrelease

func validateGraphRun(control ReleaseControl, facts journalFacts, planJSON []byte) error {
	run := control.GraphRun
	valid := run.V == 3 && run.Status == "awaiting_dispatch_authorization" &&
		validText(run.GraphRunID, 128) && validText(run.GraphID, 128) &&
		isLowerHexDigest(run.SourceSnapshotSHA256) &&
		isLowerHexDigest(run.GraphManifestSHA256) &&
		run.SchedulerProtocolVersion == control.SchedulerProtocolVersion &&
		isLowerHexDigest(run.PlanSHA256) &&
		run.PlanBytes == uint64(len(planJSON)) &&
		run.NodeCount == uint64(len(control.Plan.AuthoredNodeIDs)) &&
		run.WaveCount == uint64(len(control.Plan.Waves)) &&
		run.ExecutionContractPresent && run.DispatchRequestPresent &&
		!run.DispatchAuthorityReleased && run.LastEventSeq == 3 &&
		run.JournalBytes == facts.Bytes && validSignedTime(run.CreatedAtMS)
	if !valid || validateRunBindings(control) != nil ||
		validateEventEnvelopes(run, facts) != nil {
		return errInvalidControl
	}
	return nil
}

func validateRunBindings(control ReleaseControl) error {
	run := control.GraphRun
	valid := run.GraphID == control.Plan.GraphID &&
		run.SourceSnapshotSHA256 == control.Manifest.Source.SnapshotSHA256 &&
		run.GraphManifestSHA256 == control.Plan.GraphManifestSHA256 &&
		run.PlanSHA256 == control.Plan.PlanSHA256 &&
		run.SchedulerProtocolVersion == control.Plan.SchedulerProtocolVersion &&
		!control.Plan.ExecutionContractPresent &&
		!control.Plan.DispatchAuthorityReleased
	if !valid {
		return errInvalidControl
	}
	return nil
}

func validateEventEnvelopes(run GraphRunRecord, facts journalFacts) error {
	first := facts.Prepared
	second := facts.Contract
	third := facts.Dispatch
	valid := first.V == 1 && first.Seq == 1 && first.Type == "graph_run_prepared" &&
		first.GraphRunID == run.GraphRunID && first.GraphID == run.GraphID &&
		first.GraphManifestSHA256 == run.GraphManifestSHA256 &&
		first.PlanSHA256 == run.PlanSHA256 &&
		first.SchedulerProtocolVersion == run.SchedulerProtocolVersion &&
		first.PreparedAtMS == run.CreatedAtMS && validSignedTime(first.PreparedAtMS) &&
		second.V == 2 && second.Seq == 2 &&
		second.Type == "node_execution_contract_admitted" &&
		second.GraphRunID == run.GraphRunID &&
		second.PreviousEventSHA256 == facts.PreparedSHA256 &&
		validSignedTime(second.AdmittedAtMS) &&
		third.V == 3 && third.Seq == 3 &&
		third.Type == "node_dispatch_request_prepared" &&
		third.GraphRunID == run.GraphRunID &&
		third.PreviousEventSHA256 == facts.ContractSHA256 &&
		validSignedTime(third.PreparedAtMS)
	if !valid {
		return errInvalidControl
	}
	return nil
}
