package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/kitsch-9527/msgdb-dump/dump"
)

func main() {
	exeDir, _ := filepath.Abs(filepath.Dir(os.Args[0]))
	outDir := flag.String("out-dir", exeDir, "output directory (default: executable directory)")
	lang := flag.String("lang", "1033,2052", "comma-separated list of LCIDs (e.g. 1033,2052)")
	providers := flag.String("providers", "", "comma-separated glob patterns to filter providers (empty=all)")
	withWELM := flag.String("with-welm", "", "path to WELM seed database for import")
	update := flag.Bool("update", false, "update existing database (re-run extraction)")
	importDLL := flag.String("import-dll", "", "directory containing DLLs from remote machines to import")
	flag.Parse()

	if *outDir == "" {
		*outDir = exeDir
	}
	fmt.Printf("[info] output directory: %s\n", *outDir)

	args := &dump.DumpArgs{
		OutDir:    *outDir,
		Lang:      *lang,
		Providers: *providers,
		WithWELM:  *withWELM,
		Update:    *update,
		ImportDLL: *importDLL,
	}

	if err := dump.RunDump(args); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}
