package graphschedule

import (
	"reflect"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateSchedule(value ExecutionSchedule) error {
	if !validHeader(value) || !validFixedPolicy(value) ||
		!validNodes(value) || !validInitialFrontier(value) {
		return errInvalidControl
	}
	digest, err := scheduleDigest(value)
	if err != nil || digest != value.ScheduleSHA256 ||
		value.ScheduleID != "graph-execution-schedule-"+digest {
		return errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxExecutionScheduleBytes {
		return errInvalidControl
	}
	return nil
}

func validHeader(value ExecutionSchedule) bool {
	return value.V == ExecutionScheduleVersion && value.SchedulerProtocolVersion == 1 &&
		value.ExecutionScheduleProtocolVersion == ExecutionScheduleProtocolVersion &&
		value.ExpectedLastEventSeq == 1 && validIdentifier(value.GraphRunID) &&
		validIdentifier(value.GraphID) && isLowerHexDigest(value.ControlSnapshotSHA256) &&
		isLowerHexDigest(value.ExpectedLastEventSHA256) &&
		isLowerHexDigest(value.SourceSnapshotSHA256) &&
		isLowerHexDigest(value.GraphManifestSHA256) && isLowerHexDigest(value.CorePlanSHA256) &&
		value.NodeCount >= 2 && value.NodeCount <= 32 && value.WaveCount >= 1 &&
		value.WaveCount <= value.NodeCount && len(value.Nodes) == int(value.NodeCount) &&
		isLowerHexDigest(value.ScheduleSHA256)
}

func validFixedPolicy(value ExecutionSchedule) bool {
	return value.ExecutionMode == "serial" && value.MaxInFlightNodes == 1 &&
		value.SelectionPolicy == "topology_wave_then_authored_order" &&
		value.ProgressionPolicy == "completed_contiguous_prefix" &&
		value.AttemptPolicy == "exactly_one" && value.FailurePolicy == "fail_fast_no_retry" &&
		reflect.DeepEqual(value.OutcomePolicy, fixedOutcomePolicy()) &&
		value.PredecessorSemantics == "ordering_only" && value.PredecessorDataflow == "none" &&
		!value.PartialOutputDataflow && value.ReceiptHandling == "future_verified_identity_slots" &&
		!value.ExecutionContractPresent && !value.DispatchAuthorityReleased &&
		!value.ProgressObserved && !value.SuccessorAdvanced
}

func validNodes(value ExecutionSchedule) bool {
	identifiers := make(map[string]uint16, len(value.Nodes))
	authored := make(map[uint16]struct{}, len(value.Nodes))
	waves := make(map[uint16]struct{}, value.WaveCount)
	for index, node := range value.Nodes {
		if !validScheduledNode(node, index, value) {
			return false
		}
		if index > 0 && node.TopologyWaveIndex == value.Nodes[index-1].TopologyWaveIndex &&
			node.AuthoredNodeIndex <= value.Nodes[index-1].AuthoredNodeIndex {
			return false
		}
		if _, duplicate := identifiers[node.NodeID]; duplicate {
			return false
		}
		if _, duplicate := authored[node.AuthoredNodeIndex]; duplicate {
			return false
		}
		identifiers[node.NodeID] = node.ExecutionOrdinal
		authored[node.AuthoredNodeIndex] = struct{}{}
		waves[node.TopologyWaveIndex] = struct{}{}
	}
	return len(waves) == int(value.WaveCount) && validPredecessors(value.Nodes, identifiers)
}

func validScheduledNode(node ScheduledNode, index int, schedule ExecutionSchedule) bool {
	return node.ExecutionOrdinal == uint16(index) && validIdentifier(node.NodeID) &&
		node.AuthoredNodeIndex < schedule.NodeCount && node.TopologyWaveIndex < schedule.WaveCount &&
		isLowerHexDigest(node.ProjectLaneSHA256) && node.Attempt == 1 &&
		node.DirectPredecessorNodeIDs != nil &&
		(index == 0 || node.TopologyWaveIndex >= schedule.Nodes[index-1].TopologyWaveIndex)
}

func validPredecessors(nodes []ScheduledNode, ordinals map[string]uint16) bool {
	for _, node := range nodes {
		seen := make(map[string]struct{}, len(node.DirectPredecessorNodeIDs))
		lastAuthored := -1
		for _, predecessor := range node.DirectPredecessorNodeIDs {
			ordinal, exists := ordinals[predecessor]
			if !exists || ordinal >= node.ExecutionOrdinal ||
				nodes[ordinal].TopologyWaveIndex >= node.TopologyWaveIndex {
				return false
			}
			position := int(nodes[ordinal].AuthoredNodeIndex)
			if _, duplicate := seen[predecessor]; duplicate || position <= lastAuthored {
				return false
			}
			seen[predecessor], lastAuthored = struct{}{}, position
		}
	}
	return true
}

func validInitialFrontier(value ExecutionSchedule) bool {
	if len(value.InitialFrontier) == 0 ||
		value.InitialNode != value.Nodes[0].NodeID || value.InitialFrontier[0] != value.InitialNode {
		return false
	}
	want := make([]string, 0, len(value.InitialFrontier))
	for _, node := range value.Nodes {
		if node.TopologyWaveIndex == 0 {
			want = append(want, node.NodeID)
		}
	}
	return reflect.DeepEqual(value.InitialFrontier, want)
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func validIdentifier(value string) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" && len(value) <= 128 &&
		!strings.ContainsFunc(value, func(character rune) bool {
			return unicode.IsControl(character) || character == '\u061c' ||
				character == '\u200e' || character == '\u200f' ||
				character >= '\u2028' && character <= '\u202e' ||
				character >= '\u2066' && character <= '\u2069'
		})
}

// MarshalSchedule returns exact compact canonical JSON without a trailing LF.
func MarshalSchedule(value ExecutionSchedule) ([]byte, error) {
	if validateSchedule(value) != nil {
		return nil, errInvalidControl
	}
	return canonicalBytes(value)
}
