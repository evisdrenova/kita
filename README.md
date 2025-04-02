# Kita

A modern take on the Mac Spotlight.

Fast and intelligent search running locally on your mac.

(Kita means "found" in Japanese)

# Design

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

// Fast, local Emebdding creation and RAG

1. Parse files (txt, pdf, etc. ), normalize, then chunk large files to convert into a normalized internal representation
2. Transform chunks into embeddings for semantic search (find a local, oss model), consider quantized model using 4-bit or 8-bit, can check out rust-bert or tch-rs (use GPU if possible, fallback to CPU), load model once
3. Store the embeddings in a structure that allows for fast ANN queries like Qdrant, or even hnswlib. Store embeddings on disk and then load indexes to memory. Mayube store vectors on disk and indexes in memory
4. Pick a local LLM that can run in quantized mode and 16gb of ram, maybe llama or deepseek or qwen., Use rust-bindings in llama.cpp or ggml?? Make sure the context window i slarge enough to handle chunks/indexes

text chunking -> line by line chunking basesd '\n'
pdf chunking -> find some rust pdf parser, images??
xls -> read row by row or cell ranges

// Process

1. Parse files → chunk → embed → index in vector store.
2. Embed query → retrieve top-k relevant chunks → assemble prompt → run local LLM → return answer.
3. Re-index or add new files by repeating ingestion steps.

## Roadmap / Issues

// i might be able to use the sysinfo crate to replace all of the libproc functions - investigate further

// optimize the app list rendering it's a little slow - coudl use virtualization for the table or somethign else

// ability to create hot keys and startup flows that llow you to start up multiple apps at once or do other workflows

// results in vectordb are returned twice, maybe duplices?

// when we do the embeddings, we count all of the files that we're going to embed, but only do the ones that aren't empty, so it turns out saying something like 9 out of 60 completed, we need to return the statuses for the other ones too i.e. 9 completed, 52 skipped

// maybe we need to count tokens so we know how big the context window is getting and show that on the front-end???

// debug the LLM chat<->flow so that it's smooth and we're able to go back and forth

// then start sendign in the vectorized files into the context window so we can start to actually do that RAG
