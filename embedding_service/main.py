import grpc
from concurrent import futures
import numpy as np
from sentence_transformers import SentenceTransformer
import hnswlib
import os
from gen.pb.v1.embedding_service_pb2 import (
    EmbedTextResponse, SearchFilesResponse, SearchResult, FileData
)
from gen.pb.v1.embedding_service_pb2_grpc import (
    EmbeddingServiceServicer, add_EmbeddingServiceServicer_to_server
)
class EmbeddingService(EmbeddingServiceServicer):
    def __init__(self):
        self.INDEX_PATH = "vector_index.bin"
        self.max_elements = 10000
        
        # Load the embedding model
        self.model = SentenceTransformer("all-MiniLm-L6-v2")
        self.dim = self.model.get_sentence_embedding_dimension()
        
        # Initialize/load the vector index
        self.index = hnswlib.Index(space="cosine", dim=self.dim)
        if os.path.exists(self.INDEX_PATH):
            self.index.load_index(self.INDEX_PATH)
        else:
            self.index.init_index(max_elements=self.max_elements, ef_construction=200, M=16)
        self.index.set_ef(50)

    def EmbedText(self, request, context):
        if not request.text:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "Text cannot be empty")
        
        emb = self.model.encode(request.text)
        return EmbedTextResponse(embedding=emb.tolist())

    def SearchFiles(self, request, context):
        query_emb = self.model.encode(request.query)
        labels, distances = self.index.knn_query(np.array([query_emb]), k=request.k)
        
        results = []
        for label, distance in zip(labels[0], distances[0]):
            results.append(SearchResult(
                file_id=int(label),
                distance=float(distance)
            ))
        return SearchFilesResponse(results=results)

    def AddFile(self, request, context):
        embedding = np.array(request.embedding)
        self.index.add_items(
            np.array([embedding]), 
            np.array([request.file_id])
        )
        self.index.save_index(self.INDEX_PATH)
        return request

def serve():
    try:
        print("Starting gRPC server...")
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
        service = EmbeddingService()
        print("Initialized EmbeddingService")
        add_EmbeddingServiceServicer_to_server(service, server)
        address = '[::]:50051'
        server.add_insecure_port(address)
        print(f"Added insecure port: {address}")
        server.start()
        print("Server started successfully")
        server.wait_for_termination()
    except Exception as e:
        print(f"Failed to start server: {e}")
        raise

if __name__ == '__main__':
    print("Starting embedding service...")
    serve()