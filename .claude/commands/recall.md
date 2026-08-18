Retrieve something from persistent project memory.

Usage: /recall <key>
       /recall             (list all memories)
       /recall agent:      (list all keys starting with "agent:")

Steps:
1. If a key is provided: use agent_recall MCP tool
2. If a prefix is provided (ends with :): use agent_memories tool with prefix filter
3. If nothing is provided: use agent_memories tool to list all keys
4. Display the result

Examples:
  /recall user:name           → "Alice"
  /recall project:stage       → "MVP"
  /recall decisions:          → lists all decisions:* keys
  /recall                     → lists all stored memories
