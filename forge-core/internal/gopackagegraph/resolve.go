package gopackagegraph

import (
	"fmt"
	"strings"
)

type importResolver struct {
	compilePackages map[string][]string
	module          Module
	nested          map[string]struct{}
}

func newImportResolver(module Module, packages []Package) importResolver {
	resolver := importResolver{
		compilePackages: make(map[string][]string), module: module,
		nested: make(map[string]struct{}, len(module.NestedModules)),
	}
	for _, boundary := range module.NestedModules {
		resolver.nested[boundary.Directory] = struct{}{}
	}
	for _, item := range packages {
		if len(item.CompileFiles) != 0 {
			resolver.compilePackages[item.Directory] = append(
				resolver.compilePackages[item.Directory], item.Name)
		}
	}
	return resolver
}

func (resolver importResolver) resolve(
	importPath string,
) (string, *string, *string, *string, error) {
	if importPath == "C" {
		return "cgo_pseudo", nil, nil, nil, nil
	}
	if !canonicalImportPath(importPath) {
		return "unsupported", stringPointer("noncanonical_import_path"), nil, nil, nil
	}
	target, local, err := resolver.localTarget(importPath)
	if err != nil || !local {
		return resolver.nonlocal(importPath, err)
	}
	if resolver.withinNested(target) {
		return "nested_module_boundary", stringPointer("nested_module_boundary"),
			stringPointer(target), nil, nil
	}
	candidates := resolver.compilePackages[target]
	if len(candidates) == 0 {
		return "unresolved_local", stringPointer("no_compile_package"),
			stringPointer(target), nil, nil
	}
	if len(candidates) > 1 {
		return "ambiguous_local", stringPointer("multiple_compile_packages"),
			stringPointer(target), nil, nil
	}
	return "local", nil, stringPointer(target), stringPointer(candidates[0]), nil
}

func (resolver importResolver) localTarget(importPath string) (string, bool, error) {
	if importPath == resolver.module.ModulePath {
		return resolver.module.Directory, true, nil
	}
	prefix := resolver.module.ModulePath + "/"
	if !strings.HasPrefix(importPath, prefix) {
		return "", false, nil
	}
	target := joinDirectory(
		resolver.module.Directory, strings.TrimPrefix(importPath, prefix))
	if !safeDirectory(target) || !validText(target) {
		return "", false, fmt.Errorf("derived local target directory is outside the bounded profile")
	}
	return target, true, nil
}

func (resolver importResolver) withinNested(target string) bool {
	for current := target; current != "."; current = pathDirectory(current) {
		if _, exists := resolver.nested[current]; exists {
			return true
		}
	}
	_, exists := resolver.nested["."]
	return exists
}

func (resolver importResolver) nonlocal(
	importPath string,
	err error,
) (string, *string, *string, *string, error) {
	if err != nil {
		return "", nil, nil, nil, err
	}
	if strings.Contains(strings.SplitN(importPath, "/", 2)[0], ".") {
		return "external_candidate", nil, nil, nil, nil
	}
	return "stdlib_candidate", nil, nil, nil, nil
}
