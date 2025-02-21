package main

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	esm "github.com/evisdrenova/kita/orchestrator/service"
	_ "github.com/mattn/go-sqlite3"
)

type SearchSectionType string

type SearchCategory string

const (
	Files    SearchSectionType = "files"
	Apps     SearchSectionType = "apps"
	Semantic SearchSectionType = "semantic"
)

const (
	CategoryApplications SearchCategory = "Applications"
	CategoryPDFDocuments SearchCategory = "PDF Documents"
	CategoryDocuments    SearchCategory = "Documents"
	CategoryImages       SearchCategory = "Images"
	CategorySpreadsheets SearchCategory = "Spreadsheets"
	CategoryOther        SearchCategory = "Other"
)

type BaseMetadata struct {
	ID   *int   `json:"id,omitempty"`
	Name string `json:"name"`
	Path string `json:"path"`
}

type FileMetadata struct {
	BaseMetadata
	Type      SearchSectionType `json:"type"`
	Extension string            `json:"extension"`
	Size      int64             `json:"size"`
	UpdatedAt *string           `json:"updated_at,omitempty"`
	CreatedAt *string           `json:"created_at,omitempty"`
}

type AppMetadata struct {
	BaseMetadata
	Type        SearchSectionType `json:"type"`
	IsRunning   bool              `json:"isRunning"`
	MemoryUsage *float64          `json:"memoryUsage,omitempty"`
	CPUUsage    *float64          `json:"cpuUsage,omitempty"`
	IconDataURL *string           `json:"iconDataUrl,omitempty"`
}

type SemanticMetadata struct {
	BaseMetadata
	Type      SearchSectionType `json:"type"`
	Extension string            `json:"extension"`
	Distance  float64           `json:"distance"`
	Content   *string           `json:"content,omitempty"`
}

// handles all file processing operations
type FileProcessor struct {
	Db             *sql.DB
	TotalFiles     int
	ProcessedFiles int
	mu             sync.Mutex
	wg             sync.WaitGroup
	semaphore      chan struct{} // for limiting concurrent operations
	Embeddings     *esm.EmbeddingServiceManager
}

type ProcessingStatus struct {
	Total      int `json:"total"`
	Processed  int `json:"processed"`
	Percentage int `json:"percentage"`
}

// initializes a new file processor
func NewFileProcessor(dbPath string) (*FileProcessor, error) {
	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %v", err)
	}

	db.SetMaxOpenConns(4)
	db.SetMaxIdleConns(2)

	embedManager, err := esm.NewEmbeddingServiceManager()
	if err != nil {
		db.Close()
		return nil, fmt.Errorf("failed to start embedding service: %v", err)
	}

	return &FileProcessor{
		Db:         db,
		semaphore:  make(chan struct{}, 4), // limit to 4 concurrent ops
		Embeddings: embedManager,
	}, nil
}

// ProcessPaths processes multiple file paths concurrently
func (fp *FileProcessor) ProcessPaths(paths []string) (map[string]interface{}, error) {
	fp.TotalFiles = 0
	fp.ProcessedFiles = 0
	var allFiles []FileMetadata

	// Collect all files first
	for _, targetPath := range paths {
		if isDirectory(targetPath) {
			files, err := fp.getAllFiles(targetPath)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Error processing directory %s: %v\n", targetPath, err)
				continue
			}
			allFiles = append(allFiles, files...)
		} else {
			info, err := os.Stat(targetPath)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Error getting file info %s: %v\n", targetPath, err)
				continue
			}
			allFiles = append(allFiles, FileMetadata{
				BaseMetadata: BaseMetadata{
					Path: targetPath,
					Name: filepath.Base(targetPath),
				},
				Type:      Files,
				Extension: filepath.Ext(targetPath),
				Size:      info.Size(),
			})
		}
	}

	fmt.Fprintf(os.Stderr, "Processing %d files:\n", len(allFiles))

	fp.TotalFiles = len(allFiles)
	fp.updateProgress()

	errChan := make(chan error, len(allFiles)) // creates a buffer channel to hold errors from all concurrent runs, len set to all files in case all files have an error

	for _, file := range allFiles {
		fp.wg.Add(1)
		go func(f FileMetadata) {
			defer fp.wg.Done()                // defers until the function is done executing
			fp.semaphore <- struct{}{}        //  used to manage concurrency, sends empty struct to semaphore, if full then it can't send it and will wait until it can
			defer func() { <-fp.semaphore }() // release semaphore making space for another go routine to start

			if err := fp.processFile(f); err != nil {
				errChan <- fmt.Errorf("error processing %s: %v", f.Path, err) // send error to errChan with path
				return
			}

			fp.mu.Lock()        // lock the struct
			fp.ProcessedFiles++ // increment processed files
			fp.updateProgress() // update the progress
			fp.mu.Unlock()      // unlock the struct
		}(file)
	}

	// Wait for all goroutines to complete
	fp.wg.Wait()
	close(errChan)

	// Collect any errors
	var errors []string
	for err := range errChan {
		errors = append(errors, err.Error())
	}

	result := map[string]interface{}{
		"success":    len(errors) == 0,
		"totalFiles": fp.ProcessedFiles,
	}
	if len(errors) > 0 {
		result["errors"] = errors
	}

	return result, nil
}

// processFile handles the processing of a single file
func (fp *FileProcessor) processFile(file FileMetadata) error {
	content, err := fp.extractText(file.Path)
	if err != nil {
		if strings.Contains(err.Error(), "malformed PDF") {
			fmt.Fprintf(os.Stderr, "Skipping malformed PDF %s: %v\n", file.Path, err)
			return nil
		}
		return fmt.Errorf("failed to extract text: %v", err)
	}
	if content == "" {
		return nil
	}

	category := getCategoryFromExtension(file.Extension)

	// Create the database operation
	dbOperation := func(tx *sql.Tx) error {
		var fileID int64
		err := tx.QueryRow("SELECT id FROM files WHERE path = ?", file.Path).Scan(&fileID)
		if err == sql.ErrNoRows {
			// Insert new file
			result, err := tx.Exec(`
                INSERT INTO files (path, name, category, extension, created_at, updated_at)
                VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)`,
				file.Path, file.Name, category, file.Extension)
			if err != nil {
				return err
			}
			fileID, _ = result.LastInsertId()
		} else if err != nil {
			return err
		} else {
			// Update existing file
			_, err = tx.Exec(`
                UPDATE files 
                SET name = ?, category = ?, updated_at = CURRENT_TIMESTAMP 
                WHERE id = ?`,
				file.Name, category, fileID)
			if err != nil {
				return err
			}
		}

		// Generate embedding
		embedding, err := fp.getEmbedding(content)
		if err != nil {
			return err
		}

		embeddingJSON, err := json.Marshal(embedding)
		if err != nil {
			return err
		}

		// Update embedding
		_, err = tx.Exec(`
            INSERT OR REPLACE INTO embeddings (file_id, embedding, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)`,
			fileID, string(embeddingJSON))
		if err != nil {
			return err
		}

		// Update vector index
		err = fp.updateVectorIndex(fileID, embedding)
		if err != nil {
			return err
		}

		return nil
	}

	// Execute the operation with retries
	return fp.retryDBOperation(dbOperation)
}

// getAllFiles recursively gets all files in a directory
func (fp *FileProcessor) getAllFiles(dirPath string) ([]FileMetadata, error) {
	var files []FileMetadata
	err := filepath.Walk(dirPath, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if !info.IsDir() {
			files = append(files, FileMetadata{
				BaseMetadata: BaseMetadata{
					Path: path,
					Name: info.Name(),
				},
				Type:      Files,
				Extension: filepath.Ext(path),
				Size:      info.Size(),
			})
		}
		return nil
	})
	return files, err
}

// updateProgress prints the current progress to stdout for the Electron app to read
func (fp *FileProcessor) updateProgress() {
	if fp.TotalFiles > 0 {
		status := ProcessingStatus{
			Total:      fp.TotalFiles,
			Processed:  fp.ProcessedFiles,
			Percentage: int((float64(fp.ProcessedFiles) / float64(fp.TotalFiles)) * 100),
		}

		jsonData, err := json.Marshal(status)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error marshaling status: %v\n", err)
			return
		}

		// Only write to stdout, use stderr just for errors
		fmt.Fprintln(os.Stdout, string(jsonData))
		os.Stdout.Sync()
	}
}
