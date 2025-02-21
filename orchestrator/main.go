package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"
)

func main() {
	// Setup command line flags
	dbPath := flag.String("db", "", "Path to SQLite database")
	flag.Parse()

	if *dbPath == "" {
		fmt.Fprintf(os.Stderr, "Error: database path is required\n")
		flag.Usage()
		os.Exit(1)
	}

	// Initialize the file processor
	fp, err := NewFileProcessor(*dbPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error initializing processor: %v\n", err)
		os.Exit(1)
	}

	defer fp.Db.Close()

	// Setup signal handling for graceful shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	log.Printf("File processor started with database: %s\n", *dbPath)

	// Handle incoming paths from stdin
	decoder := json.NewDecoder(os.Stdin)
	encoder := json.NewEncoder(os.Stdout)

	for {
		select {
		case sig := <-sigChan:
			log.Printf("Received signal %v, shutting down...\n", sig)
			return
		default:
			var request struct {
				Paths []string `json:"paths"`
			}

			if err := decoder.Decode(&request); err != nil {
				log.Printf("Error decoding request: %v\n", err)
				continue
			}

			result, err := fp.ProcessPaths(request.Paths)
			if err != nil {
				log.Printf("Error processing paths: %v\n", err)
				encoder.Encode(map[string]interface{}{
					"error": err.Error(),
				})
				continue
			}

			if err := encoder.Encode(result); err != nil {
				log.Printf("Error encoding result: %v\n", err)
				continue
			}
		}
	}
}
