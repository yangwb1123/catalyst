package planningownership

import "fmt"

// Request is an opaque, immutable validated projection request.
type Request struct {
	document map[string]any
	encoded  []byte
	catalog  []byte
	mapping  []byte
}

// Projection is an opaque, immutable complete ownership projection.
type Projection struct {
	document map[string]any
	encoded  []byte
}

// CanonicalBytes returns a defensive copy of the exact compact request bytes.
func (request Request) CanonicalBytes() []byte {
	return cloneBytes(request.encoded)
}

// CatalogSourceBytes returns a defensive copy of the embedded catalog bytes.
func (request Request) CatalogSourceBytes() []byte {
	return cloneBytes(request.catalog)
}

// MappingSourceBytes returns a defensive copy of the embedded mapping bytes.
func (request Request) MappingSourceBytes() []byte {
	return cloneBytes(request.mapping)
}

// CanonicalBytes returns a defensive copy of the exact compact projection bytes.
func (projection Projection) CanonicalBytes() []byte {
	return cloneBytes(projection.encoded)
}

func (request Request) valid() error {
	if request.document == nil || len(request.encoded) == 0 || len(request.catalog) == 0 || len(request.mapping) == 0 {
		return fmt.Errorf("request is not initialized")
	}
	return nil
}

func (projection Projection) valid() error {
	if projection.document == nil || len(projection.encoded) == 0 {
		return fmt.Errorf("projection is not initialized")
	}
	return nil
}

func cloneBytes(value []byte) []byte {
	return append([]byte(nil), value...)
}

func cloneValue(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		result := make(map[string]any, len(typed))
		for key, child := range typed {
			result[key] = cloneValue(child)
		}
		return result
	case []any:
		result := make([]any, len(typed))
		for index, child := range typed {
			result[index] = cloneValue(child)
		}
		return result
	default:
		return typed
	}
}

func cloneObject(value map[string]any) map[string]any {
	return cloneValue(value).(map[string]any)
}
