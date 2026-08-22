//go:build unix

package execbound

import (
	"context"
	"os"
	"testing"
)

func TestRunPassesExtraFilesBeginningAtDescriptorThree(t *testing.T) {
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := writer.WriteString("bound-root\n"); err != nil {
		t.Fatal(err)
	}
	_ = writer.Close()
	defer reader.Close()
	result := Run(context.Background(), []string{"sh", "-c", "cat <&3"}, Options{},
		CaptureSplit, Spec{ExtraFiles: []*os.File{reader}})
	if result.Err != nil || string(result.Stdout) != "bound-root\n" {
		t.Fatalf("extra-file run = %q, %v", result.Stdout, result.Err)
	}
}

func TestRunObservedPassesExtraFilesBeginningAtDescriptorThree(t *testing.T) {
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := writer.WriteString("observed-root\n"); err != nil {
		t.Fatal(err)
	}
	_ = writer.Close()
	defer reader.Close()
	result := RunObserved(context.Background(), []string{"sh", "-c", "cat <&3"},
		Options{}, CaptureSplit, Spec{ExtraFiles: []*os.File{reader}}, ObservationOptions{})
	if result.Legacy.Err != nil || string(result.Legacy.Stdout) != "observed-root\n" {
		t.Fatalf("observed extra-file run = %q, %v", result.Legacy.Stdout, result.Legacy.Err)
	}
}
