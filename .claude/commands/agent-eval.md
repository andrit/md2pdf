Run behavioral tests on the agent's system prompt and tool usage.

Usage: /agent-eval

Steps:
1. Ask the user what scenarios to test, or offer to generate a standard suite
2. Build scenarios with user messages and expected behavior
3. Use the agent_eval MCP tool with the system prompt and scenarios
4. Report: pass rate, per-scenario results, tool usage
5. If scenarios fail: explain what the agent did wrong and suggest prompt fixes

A scenario defines user messages and expectations:
  - tool_called: "rag_query" — agent MUST call this tool
  - tool_not_called: "agent_forget" — agent must NOT call this tool
  - response_contains: ["30 days"] — response must include this
  - response_not_contains: ["I don't know"] — response must NOT include this
  - max_tool_calls: 2 — no more than 2 tool calls this turn

Example:
{
  "name": "support-bot-baseline",
  "system_prompt": "You are a helpful support agent...",
  "scenarios": [
    {
      "name": "answers-from-docs",
      "turns": [
        {"role": "user", "content": "What is the refund policy?"},
        {"expect": {"tool_called": "rag_query", "response_contains": ["30 days"]}}
      ]
    },
    {
      "name": "refuses-off-topic",
      "turns": [
        {"role": "user", "content": "What's the weather like?"},
        {"expect": {"tool_not_called": "rag_query", "response_not_contains": ["sunny", "rain"]}}
      ]
    }
  ]
}

Interpreting results:
- Pass rate 100%: all scenarios pass, agent behaves as expected
- Failed scenarios: check which checks failed and adjust the system prompt
- High tool_usage counts: agent may be making unnecessary calls
- Run after every prompt change to catch regressions
