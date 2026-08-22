package graphsnapshot

func sealEnvelope(
	request Request,
	snapshot Snapshot,
	requestJSON, snapshotJSON []byte,
	profile projectionProfile,
) (*Production, error) {
	value := Envelope{
		APIVersion: profile.apiVersion, Canonicalization: canonicalization,
		Request: request, Snapshot: snapshot,
	}
	value.EnvelopeSHA256 = ""
	preimage, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	value.EnvelopeSHA256 = domainDigest(profile.envelopeDomain, preimage)
	encoded, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	return &Production{
		envelope: value, envelopeJSON: encoded,
		requestJSON:  append([]byte(nil), requestJSON...),
		snapshotJSON: append([]byte(nil), snapshotJSON...),
	}, nil
}
