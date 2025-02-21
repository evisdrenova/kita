package service

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	pb "github.com/evisdrenova/kita/orchestrator/gen/pb/v1"
	"google.golang.org/grpc"
)

type EmbeddingServiceManager struct {
	PythonProcess *os.Process
	GrpcClient    pb.EmbeddingServiceClient
	Conn          *grpc.ClientConn
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
	execPath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("failed to get executable path: %v", err)
	}
	execDir := filepath.Dir(execPath)

	// Construct absolute path to Python script
	pythonScript := filepath.Join(execDir, "../../embedding_service/main.py")
	fmt.Printf("Starting Python service from: %s\n", pythonScript)

	cmd := exec.Command("python", pythonScript)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start Python service: %v", err)
	}
	m.PythonProcess = cmd.Process

	// Wait for the server to start
	time.Sleep(2 * time.Second)

	fmt.Printf("Python process started with PID: %d\n", m.PythonProcess.Pid)

	// Connect to the gRPC server
	conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
	if err != nil {
		m.Stop()
		return fmt.Errorf("failed to connect to gRPC server: %v", err)
	}

	m.Conn = conn
	m.GrpcClient = pb.NewEmbeddingServiceClient(conn)
	return nil
}

func (m *EmbeddingServiceManager) Stop() {
	if m.Conn != nil {
		m.Conn.Close()
	}
	if m.PythonProcess != nil {
		m.PythonProcess.Kill()
	}
}

func (m *EmbeddingServiceManager) EmbedText(text string) ([]float32, error) {
	resp, err := m.GrpcClient.EmbedText(context.Background(), &pb.EmbedTextRequest{
		Text: text,
	})
	if err != nil {
		return nil, err
	}
	return resp.Embedding, nil
}

func (m *EmbeddingServiceManager) SearchFiles(query string, k int32) ([]*pb.SearchResult, error) {

	fmt.Println("search files request:", query, k)
	resp, err := m.GrpcClient.SearchFiles(context.Background(), &pb.SearchFilesRequest{
		Query: query,
		K:     k,
	})
	if err != nil {
		return nil, err
	}
	return resp.Results, nil
}

func (m *EmbeddingServiceManager) AddFile(fileID int32, embedding []float32) error {
	_, err := m.GrpcClient.AddFile(context.Background(), &pb.AddFileRequest{
		FileId:    fileID,
		Embedding: embedding,
	})
	return err
}
