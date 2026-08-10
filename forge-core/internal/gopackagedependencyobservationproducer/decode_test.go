package gopackagedependencyobservationproducer

import (
	"strings"
	"testing"
)

func TestDecodeProductionRejectsWireDrift(t *testing.T) {
	canonical := readGoldenFixture(t).Expected.CanonicalProductionJSON
	tests := []struct {
		name string
		raw  string
	}{
		{name: "trailing newline", raw: canonical + "\n"},
		{name: "unknown field", raw: strings.Replace(canonical, "{", "{\"aaa_unknown\":true,", 1)},
		{name: "duplicate key", raw: strings.Replace(
			canonical, "{\"api_version\":", "{\"api_version\":\"duplicate\",\"api_version\":", 1,
		)},
		{name: "forbidden scalar", raw: strings.Replace(canonical, "fixture-go", "fixture-\u2028go", 1)},
		{name: "oversized key", raw: "{\"a" + strings.Repeat("a", maxJSONStringBytes) + "\":true}"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			production, err := DecodeProduction([]byte(test.raw))
			if err == nil || production != nil {
				t.Fatalf("production=%v error=%v", production, err)
			}
		})
	}
}

func TestDecodeProductionRejectsSemanticTampering(t *testing.T) {
	fixture := readGoldenFixture(t)
	tests := []struct {
		name   string
		mutate func(*ProductionPackage)
	}{
		{name: "coverage", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Coverage.RegularGoFilesParsed++
		}},
		{name: "file role", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Files[0].Role = "test"
		}},
		{name: "package import path", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Packages[0].ImportPath = nil
		}},
		{name: "dependency resolution", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Dependencies[0].Resolution = "local"
		}},
		{name: "nested module", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Module.NestedModules[0].Kind = "deleted"
		}},
		{name: "diagnostic", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Diagnostics[0].Code = "unknown"
		}},
		{name: "source binding", mutate: func(value *ProductionPackage) {
			value.GraphObservation.Source.SourceTreeSHA256 = strings.Repeat("0", 64)
		}},
		{name: "source entry", mutate: func(value *ProductionPackage) {
			value.SourceManifest.Entries[0].Bytes++
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := cloneProductionPackage(fixture.Production)
			test.mutate(&value)
			raw, err := canonicalJSON(value, maxProductionBytes)
			if err != nil {
				t.Fatal(err)
			}
			production, err := DecodeProduction(raw)
			if err == nil || production != nil {
				t.Fatalf("tamper accepted: production=%v", production)
			}
		})
	}
}
