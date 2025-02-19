// package main

// import (
//     "context"
//     "fmt"
//     "log"
//     "os"
//     "os/exec"
//     "path/filepath"
//     "time"

//     "google.golang.org/grpc"
//     pb "github.com/evisdrenova/kita/orchestrator/pb"
// )

// type EmbeddingServiceManager struct {
// 	pythonProcess *os.Process
// 	grpcClient    pb.orchestrator.protos.EmbeddingServiceClient
// 	conn  *grpc.ClientConn
// }