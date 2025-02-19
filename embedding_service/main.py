import grpc
from concurrent import futures
import numpy as np
from sentence_transformers import SentenceTransformer
import hnswlib
import os
from gen.pb.orchestrator.protos.embedding_service_pb2 import (
    EmbedResponse, SearchResponse, SearchResult, FileData
)
from gen.pb.orchestrator.protos.embedding_service_pb2_grpc import (
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
        return EmbedResponse(embedding=emb.tolist())

    def SearchFiles(self, request, context):
        query_emb = self.model.encode(request.query)
        labels, distances = self.index.knn_query(np.array([query_emb]), k=request.k)
        
        results = []
        for label, distance in zip(labels[0], distances[0]):
            results.append(SearchResult(
                file_id=int(label),
                distance=float(distance)
            ))
        return SearchResponse(results=results)

    def AddFile(self, request, context):
        embedding = np.array(request.embedding)
        self.index.add_items(
            np.array([embedding]), 
            np.array([request.file_id])
        )
        self.index.save_index(self.INDEX_PATH)
        return request

def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    add_EmbeddingServiceServicer_to_server(EmbeddingService(), server)
    server.add_insecure_port('[::]:50051')
    server.start()
    server.wait_for_termination()

if __name__ == '__main__':
    serve()