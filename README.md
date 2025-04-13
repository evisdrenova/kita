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

// maybe we need to count tokens so we know how big the context window is getting and show that on the front-end???

// implement the change_llm method so that we can swap the model if the user decides to change it in settings. if so then we need to restart the server with the new model

// support '@' action like email, message, etc.
// creates a new interface for you to do that

// if i want the LLM to be easy to use i can just use ollama and point it a local server
//https://github.com/pepperoni21/ollama-rs?tab=readme-ov-file#installation

// update the watcher to also track individual files as well in addition to the parents of the files so that we can re-index changes to files

// can't click and open sources

// update the app handler to just use native swift code and remove the objc crate

// find a way to prompt native mac permission window for contacts since it's not prompting but that's probably because we denied permission and when we do that it doesn't show it again

// i think in an initial page, we ask the user what they want to do, liek email people, message peoplem, etc. and then configure from there? or when the user does @imessage for the first time, we can prompt them then
