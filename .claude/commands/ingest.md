Ingest a document into the RAG knowledgebase.

Usage: /ingest <filepath>

Steps:
1. Use the rag_ingest MCP tool with the provided filepath
2. The tool automatically checks SHA256 hash for changes
3. Text files are processed inline; PDFs/images are queued for async processing
4. If unchanged: report "skipped" with existing document ID
5. If new/changed: report chunk count and content hash
6. The filepath is relative to /workspace/documents/ inside Docker

Example: /ingest documents/handbook.txt
→ calls rag_ingest with filepath="/workspace/documents/handbook.txt"
