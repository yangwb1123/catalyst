package goimpactprescan

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
)

func canonicalJSON(value any, maxBytes int) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("canonical encoder omitted terminal newline")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maxBytes {
		return nil, fmt.Errorf("canonical document exceeds %d bytes", maxBytes)
	}
	return append([]byte(nil), encoded...), nil
}

func domainDigest(domain string, encoded []byte) string {
	hash := sha256.New()
	hash.Write([]byte(domain))
	hash.Write([]byte{0})
	hash.Write(encoded)
	return hex.EncodeToString(hash.Sum(nil))
}

func sealRequest(value Request) (Request, []byte, error) {
	value.RequestSHA256 = ""
	preimage, err := canonicalJSON(value, maxRequestBytes)
	if err != nil {
		return Request{}, nil, err
	}
	value.RequestSHA256 = domainDigest(requestDigestDomain, preimage)
	encoded, err := canonicalJSON(value, maxRequestBytes)
	return value, encoded, err
}

func sealReport(value Report) (Report, []byte, error) {
	value.ReportSHA256 = ""
	preimage, err := canonicalJSON(value, maxReportBytes)
	if err != nil {
		return Report{}, nil, err
	}
	value.ReportSHA256 = domainDigest(reportDigestDomain, preimage)
	encoded, err := canonicalJSON(value, maxReportBytes)
	return value, encoded, err
}

func sealEnvelope(request Request, report Report) (*Production, error) {
	value := Envelope{
		APIVersion: APIVersion, Canonicalization: Canonicalization,
		EnvelopeSHA256: "", Report: report, Request: request,
	}
	preimage, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	value.EnvelopeSHA256 = domainDigest(envelopeDigestDomain, preimage)
	encoded, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	requestJSON, err := canonicalJSON(request, maxRequestBytes)
	if err != nil {
		return nil, err
	}
	reportJSON, err := canonicalJSON(report, maxReportBytes)
	if err != nil {
		return nil, err
	}
	return &Production{
		envelope: cloneEnvelope(value), envelopeJSON: encoded,
		requestJSON: requestJSON, reportJSON: reportJSON,
	}, nil
}
