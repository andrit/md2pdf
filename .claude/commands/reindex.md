Force re-embed all documents after changing the embedding model.

Usage: /reindex

WARNING: This is expensive — re-embeds every chunk in the knowledgebase.

Steps:
1. Confirm the user wants to proceed (requires confirm: true)
2. Show current embedding model from environment
3. Remind: if dimensions changed, ALTER TABLE vector column first
4. Enqueue reindex job to the rag-worker
5. Monitor progress via /status (queue depth)

Pre-requisites if dimensions changed:
  1. Edit .env: EMBEDDING_MODEL + EMBEDDING_DIMENSIONS
  2. Run in Supabase SQL Editor:
     ALTER TABLE document_chunks ALTER COLUMN embedding TYPE vector(NEW_DIMS);
     DROP FUNCTION hybrid_search;
     -- Re-create hybrid_search with new vector(NEW_DIMS) parameter
  3. Restart stack: docker compose down && docker compose up -d
  4. Then run /reindex
