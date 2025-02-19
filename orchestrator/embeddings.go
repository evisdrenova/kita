package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
)

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
