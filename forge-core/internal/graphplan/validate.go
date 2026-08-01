package graphplan

import (
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/dependency"
)

func validateAndPlan(spec Spec) ([][]string, error) {
	if spec.V != SpecVersion || spec.Nodes == nil || spec.Edges == nil {
		return nil, errInvalidSpec
	}
	if !validText(spec.Manager.AgentProfile, maxAgentProfileBytes) ||
		!validProse(spec.Manager.Instruction, maxProseBytes) {
		return nil, errInvalidSpec
	}
	if len(spec.Nodes) < 1 || len(spec.Nodes) > maxNodes || len(spec.Edges) > maxEdges {
		return nil, errInvalidSpec
	}
	if err := validateNodes(spec.Nodes); err != nil {
		return nil, err
	}
	if err := validateEdges(spec.Edges); err != nil {
		return nil, err
	}
	if encoded, err := encodeCanonical(spec); err != nil || len(encoded) > MaxSpecBytes {
		return nil, errInvalidSpec
	}
	return dependencyWaves(spec.Nodes, canonicalEdges(spec.Edges))
}

func validateNodes(nodes []Node) error {
	seen := make(map[string]struct{}, len(nodes))
	for _, node := range nodes {
		valid := validText(node.NodeID, maxIdentifierBytes) &&
			validText(node.ProjectID, maxIdentifierBytes) &&
			validText(node.MemberRole, maxMemberRoleBytes) &&
			validText(node.AgentProfile, maxAgentProfileBytes) &&
			validProse(node.Task, maxProseBytes) &&
			validProse(node.Acceptance, maxProseBytes)
		if _, duplicate := seen[node.NodeID]; !valid || duplicate {
			return errInvalidSpec
		}
		seen[node.NodeID] = struct{}{}
	}
	return nil
}

func validateEdges(edges []Edge) error {
	seen := make(map[Edge]struct{}, len(edges))
	for _, edge := range edges {
		if _, duplicate := seen[edge]; duplicate {
			return errInvalidSpec
		}
		seen[edge] = struct{}{}
	}
	return nil
}

func dependencyWaves(nodes []Node, edges []Edge) ([][]string, error) {
	positions := make(map[string]int, len(nodes))
	dependencies := make([]dependency.Node, len(nodes))
	for position, node := range nodes {
		positions[node.NodeID] = position
		dependencies[position].ID = node.NodeID
	}
	for _, edge := range edges {
		target, exists := positions[edge.ToNodeID]
		if !exists {
			return nil, errInvalidSpec
		}
		dependencies[target].Dependencies = append(
			dependencies[target].Dependencies,
			edge.FromNodeID,
		)
	}
	indices, err := dependency.Waves(dependencies)
	if err != nil {
		return nil, errInvalidSpec
	}
	return waveIdentifiers(nodes, indices), nil
}

func waveIdentifiers(nodes []Node, indices [][]int) [][]string {
	waves := make([][]string, len(indices))
	for waveIndex, positions := range indices {
		waves[waveIndex] = make([]string, len(positions))
		for nodeIndex, position := range positions {
			waves[waveIndex][nodeIndex] = nodes[position].NodeID
		}
	}
	return waves
}

func validText(value string, maxBytes int) bool {
	return utf8.ValidString(value) &&
		strings.TrimSpace(value) != "" &&
		len(value) <= maxBytes &&
		!strings.ContainsFunc(value, unsupportedCharacter)
}

func validProse(value string, maxBytes int) bool {
	return utf8.ValidString(value) &&
		strings.TrimSpace(value) != "" &&
		len(value) <= maxBytes &&
		!strings.ContainsFunc(value, unsupportedProseCharacter)
}

func unsupportedProseCharacter(value rune) bool {
	return (unicode.IsControl(value) && value != '\n' && value != '\r' && value != '\t') ||
		isBidiControl(value)
}

func unsupportedCharacter(value rune) bool {
	return unicode.IsControl(value) || isBidiControl(value)
}

func isBidiControl(value rune) bool {
	return value == '\u061c' ||
		value == '\u200e' ||
		value == '\u200f' ||
		(value >= '\u2028' && value <= '\u202e') ||
		(value >= '\u2066' && value <= '\u2069')
}
