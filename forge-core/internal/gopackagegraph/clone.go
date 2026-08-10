package gopackagegraph

func cloneRegularFile(value RegularFile) RegularFile {
	value.Content = append([]byte(nil), value.Content...)
	return value
}

func cloneNestedModules(values []NestedModule) []NestedModule {
	return append([]NestedModule{}, values...)
}

func cloneDependencies(values []Dependency) []Dependency {
	result := make([]Dependency, len(values))
	for index, value := range values {
		value.ResolutionDetail = cloneString(value.ResolutionDetail)
		value.SourcePaths = append([]string{}, value.SourcePaths...)
		value.TargetDirectory = cloneString(value.TargetDirectory)
		value.TargetPackageName = cloneString(value.TargetPackageName)
		result[index] = value
	}
	return result
}

func cloneFiles(values []File) []File {
	result := make([]File, len(values))
	for index, value := range values {
		value.Imports = append([]string{}, value.Imports...)
		result[index] = value
	}
	return result
}

func cloneModule(value Module) Module {
	value.NestedModules = cloneNestedModules(value.NestedModules)
	return value
}

func clonePackages(values []Package) []Package {
	result := make([]Package, len(values))
	for index, value := range values {
		value.CompileFiles = append([]string{}, value.CompileFiles...)
		value.ImportPath = cloneString(value.ImportPath)
		value.TestFiles = append([]string{}, value.TestFiles...)
		result[index] = value
	}
	return result
}

func CloneObservation(value Observation) Observation {
	value.Dependencies = cloneDependencies(value.Dependencies)
	value.Diagnostics = append([]Diagnostic{}, value.Diagnostics...)
	value.Files = cloneFiles(value.Files)
	value.Module = cloneModule(value.Module)
	value.Packages = clonePackages(value.Packages)
	return value
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	return stringPointer(*value)
}

func validProducer(value Producer) bool {
	return validHash(value.ParametersSHA256) && validText(value.ProducerID) &&
		validText(value.ProducerType) && validText(value.ProducerVersion) && validText(value.RunID)
}

func validSource(value Source) bool {
	return validText(value.SourceRevision) && validHash(value.SourceTreeSHA256)
}
