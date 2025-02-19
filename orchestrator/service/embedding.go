package main

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	pb "github.com/evisdrenova/kita/orchestrator/gen/pb"
	"google.golang.org/grpc"
)

type EmbeddingServiceManager struct {
	pythonProcess *os.Process
	grpcClient    pb.EmbeddingServiceClient
	conn          *grpc.ClientConn
}

// creates a new embedding service manager
func NewEmbeddingServiceManager() (*EmbeddingServiceManager, error) {
	manager := &EmbeddingServiceManager{}
	if err := manager.Start(); err != nil {
		return nil, err
	}
	return manager, nil
}

func (m *EmbeddingServiceManager) Start() error {
	// Start Python gRPC server
	pythonScript := filepath.Join("python", "embedding_service.py")
	cmd := exec.Command("python", pythonScript)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start Python service: %v", err)
	}
	m.pythonProcess = cmd.Process

	// Wait for the server to start
	time.Sleep(2 * time.Second)

	// Connect to the gRPC server
	conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
	if err != nil {
		m.Stop()
		return fmt.Errorf("failed to connect to gRPC server: %v", err)
	}

	m.conn = conn
	m.grpcClient = pb.NewEmbeddingServiceClient(conn)
	return nil
}

func (m *EmbeddingServiceManager) Stop() {
	if m.conn != nil {
		m.conn.Close()
	}
	if m.pythonProcess != nil {
		m.pythonProcess.Kill()
	}
}

func (m *EmbeddingServiceManager) EmbedText(text string) ([]float32, error) {
	resp, err := m.grpcClient.EmbedText(context.Background(), &pb.EmbedRequest{
		Text: text,
	})
	if err != nil {
		return nil, err
	}
	return resp.Embedding, nil
}

func (m *EmbeddingServiceManager) SearchFiles(query string, k int32) ([]*pb.SearchResult, error) {
	resp, err := m.grpcClient.SearchFiles(context.Background(), &pb.SearchRequest{
		Query: query,
		K:     k,
	})
	if err != nil {
		return nil, err
	}
	return resp.Results, nil
}

func (m *EmbeddingServiceManager) AddFile(fileID int32, embedding []float32) error {
	_, err := m.grpcClient.AddFile(context.Background(), &pb.FileData{
		FileId:    fileID,
		Embedding: embedding,
	})
	return err
}
