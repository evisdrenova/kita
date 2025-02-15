# Kita

The best local search app for your mac

## Roadmap

- embedding db/ RAG pipeline
- real time mode to handle new files
- customizable hot key
- everything has to be very very fast

# Built-in embedding pipeline

1. Kita tries to create embeddings from all of the files that you have given it access to.
2. it reads the files, creates an embedding to each file and then stores that embedding in sqllite
3. then when you query, it searches HNSW for the ANN and returns the id
4. we then resolve that id to a file that we return the link to
5.
