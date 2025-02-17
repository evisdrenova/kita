package fileprocessor

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/ledongthuc/pdf"
	_ "github.com/mattn/go-sqlite3"
	"github.com/unidoc/unioffice/document"
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

	return &FileProcessor{
		Db:        db,
		semaphore: make(chan struct{}, 4), // limit to 4 concurrent ops, but we can be smarter about this
	}, nil
}

// ProcessPaths processes multiple file paths concurrently
func (fp *FileProcessor) ProcessPaths(paths []string) (map[string]interface{}, error) {
	fp.TotalFiles = 0
	fp.ProcessedFiles = 0
	var allFiles []FileMetadata

	// Collect all files first
	for _, targetPath := range paths {
		if fp.isDirectory(targetPath) {
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
		return fmt.Errorf("failed to extract text: %v", err)
	}
	if content == "" {
		return nil
	}

	category := getCategoryFromExtension(file.Extension)

	// start a transaction
	tx, err := fp.Db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	// Check if file exists
	var fileID int64
	err = tx.QueryRow("SELECT id FROM files WHERE path = ?", file.Path).Scan(&fileID)
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

	// commit the transaction
	return tx.Commit()
}

func (fp *FileProcessor) extractText(filePath string) (string, error) {
	ext := strings.ToLower(filepath.Ext(filePath))

	switch {
	case isPlainText(ext):
		return fp.extractTextFromPlain(filePath)
	case ext == ".pdf":
		return fp.extractTextFromPDF(filePath)
	case ext == ".docx":
		return fp.extractTextFromDOCX(filePath)
	default:
		return "", nil
	}
}

func isPlainText(ext string) bool {
	plainTextExts := map[string]bool{
		".txt": true, ".js": true, ".ts": true,
		".jsx": true, ".tsx": true, ".py": true,
		".java": true, ".cpp": true, ".html": true,
		".css": true, ".json": true, ".xml": true,
		".yaml": true, ".yml": true,
	}
	return plainTextExts[ext]
}

func (fp *FileProcessor) extractTextFromPlain(filePath string) (string, error) {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return "", err
	}
	return string(content), nil
}

func (fp *FileProcessor) extractTextFromPDF(filePath string) (string, error) {
	f, r, err := pdf.Open(filePath)
	if err != nil {
		return "", err
	}
	defer f.Close()

	var buf bytes.Buffer
	b, err := r.GetPlainText()
	if err != nil {
		return "", err
	}
	_, err = buf.ReadFrom(b)
	if err != nil {
		return "", err
	}
	return buf.String(), nil
}

func (fp *FileProcessor) extractTextFromDOCX(filePath string) (string, error) {
	doc, err := document.Open(filePath)
	if err != nil {
		return "", err
	}
	var text strings.Builder
	for _, para := range doc.Paragraphs() {
		for _, run := range para.Runs() {
			text.WriteString(run.Text())
		}
		text.WriteString("\n")
	}
	return text.String(), nil
}

// getEmbedding gets embedding from the Python microservice
func (fp *FileProcessor) getEmbedding(text string) ([]float64, error) {
	payload := map[string]string{"text": text}
	jsonData, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}

	resp, err := http.Post("http://127.0.0.1:8000/embed", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result struct {
		Embedding []float64 `json:"embedding"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, err
	}

	return result.Embedding, nil
}

// updateVectorIndex updates the vector index in the Python microservice
func (fp *FileProcessor) updateVectorIndex(fileID int64, embedding []float64) error {
	payload := map[string]interface{}{
		"file_id":   fileID,
		"embedding": embedding,
	}
	jsonData, err := json.Marshal(payload)
	if err != nil {
		return err
	}

	resp, err := http.Post("http://127.0.0.1:8000/add_file", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("failed to update vector index: %s", body)
	}

	return nil
}

// checks if the path is a directory
func (f *FileProcessor) isDirectory(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return info.IsDir()
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
		json.NewEncoder(os.Stdout).Encode(status)
	}
}

func getCategoryFromExtension(extension string) SearchCategory {
	if extension == "" {
		return CategoryOther
	}

	switch strings.ToLower(extension) {
	case ".app", ".exe", ".dmg":
		return CategoryApplications

	case ".pdf":
		return CategoryPDFDocuments

	case ".doc", ".docx", ".txt", ".rtf":
		return CategoryDocuments

	case ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp":
		return CategoryImages

	case ".js", ".ts", ".jsx", ".tsx", ".py", ".java", ".cpp",
		".html", ".css", ".json", ".xml", ".yaml", ".yml":
		return CategoryDocuments

	case ".mp4", ".mov", ".avi", ".mkv":
		return CategoryOther

	case ".mp3", ".wav", ".flac", ".m4a":
		return CategoryOther

	case ".xlsx", ".xls", ".csv":
		return CategorySpreadsheets

	case ".zip", ".rar", ".7z", ".tar", ".gz":
		return CategoryOther

	default:
		return CategoryOther
	}
}
