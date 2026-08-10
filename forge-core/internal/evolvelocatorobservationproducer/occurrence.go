package evolvelocatorobservationproducer

import (
	"fmt"

	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/evolvescan"
)

type occurrence struct {
	dimension     string
	evidence      evolvescan.Evidence
	opportunityID *string
	relation      string
}

type fileFact struct {
	bytes  int64
	sha256 string
}

func reportOccurrences(report evolvescan.Report) ([]occurrence, error) {
	result := make([]occurrence, 0)
	for _, dimension := range report.Dimensions {
		if dimension.Status == evolvescan.StatusUnavailable {
			continue
		}
		for _, evidence := range dimension.Evidence {
			result = append(result, occurrence{
				dimension: dimension.Name, evidence: evidence, relation: dimension.Status,
			})
		}
	}
	for _, opportunity := range report.Opportunities {
		id := opportunity.ID
		for _, evidence := range opportunity.Evidence {
			result = append(result, occurrence{
				dimension: opportunity.Dimension, evidence: evidence,
				opportunityID: stringPointer(id), relation: "opportunity",
			})
		}
	}
	if len(result) > maxObservations {
		return nil, fmt.Errorf("evolve locator occurrences exceed %d", maxObservations)
	}
	return result, nil
}

func buildObservations(
	occurrences []occurrence,
	facts map[string]fileFact,
	parametersSHA256, reportSHA256, runID, revision, treeSHA256 string,
	observedAtUnixMS int64,
) ([]locatorcontract.Observation, error) {
	observations := make([]locatorcontract.Observation, 0, len(occurrences))
	for _, item := range occurrences {
		fact, exists := facts[item.evidence.Path]
		if !exists {
			return nil, fmt.Errorf("locator path %q lacks a captured file fact", item.evidence.Path)
		}
		observations = append(observations, locatorcontract.Observation{
			APIVersion: locatorcontract.ObservationAPIVersion, Canonicalization: Canonicalization,
			Content: locatorcontract.Content{Bytes: fact.bytes, SHA256: fact.sha256},
			Locator: locatorcontract.Locator{
				Detail: item.evidence.Detail, Line: int64(item.evidence.Line), Path: item.evidence.Path,
			},
			ObservedAtUnixMS: observedAtUnixMS,
			Producer: locatorcontract.Producer{
				ParametersSHA256: parametersSHA256, ProducerID: ProducerID,
				ProducerType: "tool", ProducerVersion: ProducerVersion, RunID: runID,
			},
			ScanContext: locatorcontract.ScanContext{
				Contract: evolvescan.ContractV1, Depth: "", Dimension: item.dimension,
				OpportunityID: cloneString(item.opportunityID), Relation: item.relation,
				ReportSHA256: reportSHA256,
			},
			Source: locatorcontract.Source{
				SourceRevision: revision, SourceTreeSHA256: treeSHA256,
			},
		})
	}
	return observations, nil
}

func setObservationDepth(observations []locatorcontract.Observation, depth string) {
	for index := range observations {
		observations[index].ScanContext.Depth = depth
	}
}

func stringPointer(value string) *string { return &value }

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	return stringPointer(*value)
}
