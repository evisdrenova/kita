# main file that runs and the embedding model

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from sentence_transformers import SentenceTransformer
import hnswlib
import numpy as np
import os

class EmbedRequest(BaseModel):
    text: str

class EmbedResponse(BaseModel):
    embedding: list

class SearchRequest(BaseModel):
    query: str
    k: int = 5  # number of results

class SearchResult(BaseModel):
    file_id: int
    distance: float

class SearchResponse(BaseModel):
    results: list[SearchResult]

# configuration & persistence path
INDEX_PATH = "vector_index.bin"
max_elements = 10000

# load the embedding model
model = SentenceTransformer("all-MiniLm-L6-v2")
dim = model.get_sentence_embedding_dimension()

# initialize/load the vector index
index = hnswlib.Index(space="cosine", dim=dim)
if os.path.exists(INDEX_PATH):
    index.load_index(INDEX_PATH)
else:
    index.init_index(max_elements=max_elements, ef_construction=200, M=16)
index.set_ef(50)  # ef parameter for runtime

app = FastAPI()

# --- Endpoint: Get Embedding for a Given Text ---
@app.post("/embed", response_model=EmbedResponse)
async def embed_text(request: EmbedRequest):
    if not request.text:
        raise HTTPException(status_code=400, detail="Text cannot be empty.")
    emb = model.encode(request.text)
    return EmbedResponse(embedding=emb.tolist())

# --- Endpoint: Search for Similar Files ---
@app.post("/search", response_model=SearchResponse)
async def search_files(request: SearchRequest):
    query_emb = model.encode(request.query)
    labels, distances = index.knn_query(np.array([query_emb]), k=request.k)
    results = []
    for label, distance in zip(labels[0], distances[0]):
        results.append(SearchResult(file_id=int(label), distance=float(distance)))
    return SearchResponse(results=results)

# --- Endpoint: Save/Update a File's Embedding into the Vector Index ---
class FileData(BaseModel):
    file_id: int  # for new files, this can be 0 or -1
    embedding: list

@app.post("/add_file", response_model=FileData)
async def add_file(file: FileData):
    # In a production scenario, you'll want to ensure unique file_id assignment.
    # Here we assume the client (your Electron app) manages file_id assignment.
    index.add_items(np.array([file.embedding]), np.array([file.file_id]))
    index.save_index(INDEX_PATH)
    return file

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)