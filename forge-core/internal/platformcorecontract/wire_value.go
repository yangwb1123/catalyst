package platformcorecontract

import contractwire "forgeos/forge-core/internal/platformcorecontract/internal/wireprofile"

func typedCanonical(value any, maximum int) ([]byte, error) {
	return contractwire.TypedCanonical(value, maximum)
}

func decodeTypedCanonical(data []byte, maximum int, target any) error {
	if err := contractwire.DecodeTypedCanonical(data, maximum, target); err != nil {
		return err
	}
	return normalizeDynamicFields(target)
}

func normalizeDynamicFields(target any) error {
	switch value := target.(type) {
	case *CommandEnvelope:
		return normalizeEnvelopeMaps(&value.Payload, &value.Extensions)
	case *EventEnvelope:
		return normalizeEnvelopeMaps(&value.Payload, &value.Extensions)
	default:
		return nil
	}
}

func normalizeEnvelopeMaps(payload, extensions *map[string]any) error {
	for _, value := range []*map[string]any{payload, extensions} {
		if *value == nil {
			continue
		}
		normalized, err := contractwire.NormalizeJSONNumbers(*value)
		if err != nil {
			return err
		}
		*value = normalized.(map[string]any)
	}
	return nil
}
