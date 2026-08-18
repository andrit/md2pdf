Query the knowledgebase using hybrid search.

Usage: /query <your question>

Steps:
1. Use the rag_query MCP tool with the question text
2. Hybrid search combines cosine similarity (70%) + keyword ts_rank (30%)
3. Claude generates an answer from retrieved context chunks
4. Show the answer with hybrid scores for transparency

Example: /query What does error E-4012 mean?
