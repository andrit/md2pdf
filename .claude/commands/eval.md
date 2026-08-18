Evaluate search quality by running test queries against the knowledgebase.

Usage: /eval

Steps:
1. Ask the user what queries to test, or offer to generate a standard set
2. Build a query set with questions and expected keywords
3. Use the rag_eval MCP tool with a descriptive name and the query set
4. Report results: pass rate, MRR, per-query scores
5. Suggest improvements if scores are low (re-chunk, different model, adjust weights)

Example query set:
{
  "name": "baseline-v1",
  "queries": [
    {"question": "What is the refund policy?", "expected_keywords": ["refund", "30 days"]},
    {"question": "How do I reset my password?", "expected_keywords": ["password", "reset"]},
    {"question": "What are the rate limits?", "expected_keywords": ["rate", "limit"]}
  ]
}

Interpreting results:
- MRR > 0.8 = excellent retrieval
- MRR 0.5-0.8 = good, may benefit from tuning
- MRR < 0.5 = poor, consider reindexing or changing embedding model
- Failed queries = chunks don't contain the answer (need more docs or better chunking)
