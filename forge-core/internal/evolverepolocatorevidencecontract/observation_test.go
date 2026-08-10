package evolverepolocatorevidencecontract

import (
	"bytes"
	"testing"
)

func TestCanonicalObservationJSONMatchesAdaptationBytes(t *testing.T) {
	request := validRequest()
	got, err := CanonicalObservationJSON(request.Observation)
	if err != nil {
		t.Fatal(err)
	}
	adaptation, err := Adapt(request)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, adaptation.ObservationJSON()) {
		t.Fatalf("standalone observation bytes differ from adapter bytes\ngot:  %s\nwant: %s", got, adaptation.ObservationJSON())
	}
	got[0] ^= 0xff
	again, err := CanonicalObservationJSON(request.Observation)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(got, again) {
		t.Fatal("canonical observation result aliases prior caller mutation")
	}
}

func TestCanonicalObservationJSONRejectsInvalidObservation(t *testing.T) {
	observation := validRequest().Observation
	observation.ScanContext.ReportSHA256 = "invalid"
	if _, err := CanonicalObservationJSON(observation); err == nil {
		t.Fatal("invalid standalone observation was accepted")
	}
}
