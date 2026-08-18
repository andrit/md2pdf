Store something in persistent project memory.

Usage: /remember <key> <value>

This stores structured state that survives across Claude Code sessions.
Unlike RAG (unstructured search), memory is key-value (exact retrieval).

Steps:
1. Use the agent_remember MCP tool with the key and value
2. Confirm what was stored
3. Mention that /recall <key> retrieves it later

Examples:
  /remember user:name Alice
  /remember project:stage MVP
  /remember api:base-url https://api.example.com/v2
  /remember decisions:auth We chose JWT over sessions because...

Keys support namespacing with colons: agent:, user:, project:, decisions:
Use /recall <key> to retrieve. Use agent_memories tool to list all keys.
