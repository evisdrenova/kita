package main

import (
	"encoding/json"
	"fmt"
	"os"

	fileprocessor "github.com/evisdrenova/kita/go-orchestrator/file-processor"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintf(os.Stderr, "Usage: %s <database_path> <file_paths...>\n", os.Args[0])
		os.Exit(1)
	}

	dbPath := os.Args[1]
	paths := os.Args[2:]

	fp, err := fileprocessor.NewFileProcessor(dbPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error initializing processor: %v\n", err)
		os.Exit(1)
	}
	defer fp.Db.Close()

	result, err := fp.ProcessPaths(paths)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error processing paths: %v\n", err)
		os.Exit(1)
	}

	json.NewEncoder(os.Stdout).Encode(result)
}
