# Kita

A modern take on the Mac Spotlight.

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

// optimize the get_app_icon to be faster, the icon conversion is taking way too long
// i might be able to use the sysinfo crate to replace all of the libproc functions - investigate further
// consider replacing the local sqlite implemnentation with tantivity - it's super fast for big datasets, doesn't block writes to a single thread but it's also complicaed and i'd probably have to re-implemnent my indexing logic but check it out
// use sqlite FTS5 for fast search

## Trigram Tokenizer

Instead of standard unicode61, you provide a custom tokenizer that, for each string (like "example.pdf"), generates overlapping 3-character tokens:
"exa", "xam", "amp", "mpl", "ple", "le.", "e.p", ".pd", "pdf", etc.
Each path or filename is thus stored as a set of these “trigrams” in your FTS index.
Query

When a user types "exa", your code transforms it into something that FTS can match (often just 'exa' if you store exact trigrams, or 'exa\*' for a prefix approach).
FTS looks up documents whose trigram set includes 'exa'. This will include "example.pdf", "bexas.pdf", etc.

## Index Size

For each string of length m, you store roughly m trigram tokens (some overhead). This is the cost of enabling truly arbitrary substring search at high speed.
For a typical dataset of tens or even hundreds of thousands of files, this is often quite manageable in practice.

## Indexing

We create a small TrigramTokenizer that splits each string into 3‑character overlapping tokens (“trigrams”).
We register that tokenizer with SQLite’s FTS5 engine.
We store each filename/path as a series of these 3‑char tokens in the files_fts table.
When the user searches for, e.g., "exa", that becomes an FTS search for the exa token—matching anywhere that has those three characters consecutively.
