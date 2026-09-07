//go:build !windows

package dump

import (
	"fmt"
	"runtime"
)

// RunDump returns an error on non-Windows platforms.
func RunDump(args *DumpArgs) error {
	return fmt.Errorf("the dump command requires Windows registry access and is not available on %s", runtime.GOOS)
}

// GetOSInfo returns a default OsInfo on non-Windows.
func GetOSInfo() OsInfo {
	arch := runtime.GOARCH
	return OsInfo{
		ProductName:    runtime.GOOS,
		DisplayVersion: "0.0",
		BuildNumber:    "0",
		Architecture:   &arch,
	}
}
