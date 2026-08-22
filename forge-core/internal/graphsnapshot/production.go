package graphsnapshot

import "encoding/json"

func (value *Production) JSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.envelopeJSON...)
}

func (value *Production) RequestJSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.requestJSON...)
}

func (value *Production) SnapshotJSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.snapshotJSON...)
}

func (value *Production) Envelope() Envelope {
	if value == nil {
		return Envelope{}
	}
	var result Envelope
	if err := json.Unmarshal(value.envelopeJSON, &result); err != nil {
		return Envelope{}
	}
	return result
}

func (value *Production) SHA256() string {
	if value == nil {
		return ""
	}
	return value.envelope.EnvelopeSHA256
}

func (value *Production) RequestSHA256() string {
	if value == nil {
		return ""
	}
	return value.envelope.Request.RequestSHA256
}

func (value *Production) SnapshotSHA256() string {
	if value == nil {
		return ""
	}
	return value.envelope.Snapshot.SnapshotSHA256
}
