# Kita

Fast and intelligent search running locally on your mac.

(Kita means "found" in Japanese)

# Architecture

![alt text](image.png)

## Components

1. Electron Frontend Layer

   - UI Components: User interface elements
   - IPC Bridge: Handles communication between frontend and backend
   - State Management: Manages application state and UI updates

2. Go Orchestrator Layer

   - Request Router: Handles incoming requests from the frontend
   - File Processor: Handles file operations and indexing
   - Cache Manager: Manages in-memory caching for performance
   - Worker Pool Manager: Coordinates Python workers

3. Python Workers Layer

   - Embedding Worker: Generates embeddings for files
   - LLM Worker: Handles LLM operations
   - Vector Store: Manages vector embeddings

4. Storage Layer
   - SQLite: Persistent storage for metadata and embeddings
   - File System: Raw file storage and access

// TODO: build script to compile the go orchestrator into a binary, main.ts logic to call go binary, stdout and stderr communication channels
