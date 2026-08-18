#!/usr/bin/env python3
"""
PreCompact Hook — preserve conversation history before context compaction.

Called by Claude Code's PreCompact hook event. Receives JSON on stdin
with session_id, transcript_path, and other metadata.

What it does:
  1. Reads the full transcript from the JSONL file
  2. Saves a copy to /workspace/.workbench/transcripts/
  3. Extracts user messages and assistant responses
  4. Creates a structured summary (key decisions, tasks, state)
  5. Stores the summary in workbench memory (session:compact-summary)
  6. Stores the conversation in the conversations API
  7. Writes a recovery note so PostCompact can guide Claude

The transcript survives compaction in three places:
  - Raw file: .workbench/transcripts/{timestamp}.jsonl
  - Conversations API: searchable via conversation_get
  - Memory: session:compact-summary, session:working-on
"""
import json
import os
import sys
import shutil
from datetime import datetime
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.error import URLError

API_URL = os.environ.get("API_URL", "http://mcp-server:3100")
PROJECT = os.environ.get("WORKBENCH_PROJECT", "")

def api(path, method="GET", body=None):
    """Call the workbench API."""
    headers = {"Content-Type": "application/json"}
    if PROJECT:
        headers["X-Project"] = PROJECT
    data = json.dumps(body).encode() if body else None
    req = Request(f"{API_URL}{path}", data=data, headers=headers, method=method)
    try:
        with urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except (URLError, Exception) as e:
        print(f"API call failed: {path} — {e}", file=sys.stderr)
        return None

def read_transcript(path):
    """Read a JSONL transcript file. Each line is a JSON object."""
    messages = []
    try:
        with open(path, "r") as f:
            for line in f:
                line = line.strip()
                if line:
                    try:
                        messages.append(json.loads(line))
                    except json.JSONDecodeError:
                        continue
    except FileNotFoundError:
        print(f"Transcript not found: {path}", file=sys.stderr)
    return messages

def extract_conversation(transcript):
    """Extract user/assistant message pairs from the transcript."""
    conversation = []
    for entry in transcript:
        msg_type = entry.get("type", "")
        role = entry.get("role", "")

        if role == "user" or msg_type == "human":
            content = entry.get("content", "")
            if isinstance(content, list):
                content = " ".join(
                    block.get("text", "") for block in content
                    if isinstance(block, dict) and block.get("type") == "text"
                )
            if content and len(content.strip()) > 0:
                conversation.append({"role": "user", "content": content.strip()[:2000]})

        elif role == "assistant" or msg_type == "assistant":
            content = entry.get("content", "")
            if isinstance(content, list):
                content = " ".join(
                    block.get("text", "") for block in content
                    if isinstance(block, dict) and block.get("type") == "text"
                )
            if content and len(content.strip()) > 0:
                conversation.append({"role": "assistant", "content": content.strip()[:2000]})

    return conversation

def extract_tool_calls(transcript):
    """Extract tool calls made during the session."""
    tools = []
    for entry in transcript:
        if entry.get("type") == "tool_use" or "tool_use" in str(entry.get("content", "")):
            name = entry.get("name", "")
            if not name and isinstance(entry.get("content", ""), list):
                for block in entry["content"]:
                    if isinstance(block, dict) and block.get("type") == "tool_use":
                        name = block.get("name", "unknown")
            if name:
                tools.append(name)
    return tools

def build_summary(conversation, tool_calls):
    """Build a structured summary of the session."""
    user_msgs = [m for m in conversation if m["role"] == "user"]
    assistant_msgs = [m for m in conversation if m["role"] == "assistant"]

    # First and last user messages give the arc of the session
    first_ask = user_msgs[0]["content"][:300] if user_msgs else "Unknown"
    last_ask = user_msgs[-1]["content"][:300] if len(user_msgs) > 1 else first_ask

    # Last assistant response is likely the current state
    last_response = assistant_msgs[-1]["content"][:500] if assistant_msgs else "Unknown"

    summary = {
        "compacted_at": datetime.utcnow().isoformat() + "Z",
        "total_turns": len(conversation),
        "user_messages": len(user_msgs),
        "assistant_messages": len(assistant_msgs),
        "tool_calls": list(set(tool_calls)),
        "tool_call_count": len(tool_calls),
        "session_started_with": first_ask,
        "last_user_request": last_ask,
        "last_assistant_state": last_response,
    }
    return summary

def save_transcript_file(transcript, session_id):
    """Save raw transcript to .workbench/transcripts/."""
    transcript_dir = Path("/workspace/.workbench/transcripts")
    transcript_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.utcnow().strftime("%Y-%m-%d-%H-%M-%S")
    filename = f"{timestamp}-{session_id[:8]}.jsonl"
    filepath = transcript_dir / filename

    with open(filepath, "w") as f:
        for entry in transcript:
            f.write(json.dumps(entry) + "\n")

    return str(filepath)

def save_to_conversations_api(conversation, session_id):
    """Store the conversation in the workbench conversations API."""
    if not PROJECT:
        return None

    timestamp = datetime.utcnow().strftime("%Y-%m-%d %H:%M")
    result = api("/conversations", "POST", {
        "title": f"Session {timestamp} (pre-compaction)",
        "metadata": {"session_id": session_id, "type": "compaction-backup"},
    })

    if result and "id" in result:
        conv_id = result["id"]
        # Add messages in batches (don't overwhelm the API)
        for msg in conversation[:100]:  # Cap at 100 messages
            api(f"/conversations/{conv_id}/messages", "POST", {
                "role": msg["role"],
                "content": msg["content"],
            })
        return conv_id
    return None

def save_to_memory(summary):
    """Save the summary to workbench memory for quick recall."""
    if not PROJECT:
        return

    api("/memory/session:compact-summary", "PUT", {
        "value": json.dumps(summary),
    })

    # Save the last working state separately for easy /recall
    api("/memory/session:working-on", "PUT", {
        "value": summary.get("last_user_request", "Unknown"),
    })

def main():
    # Read hook input from stdin
    try:
        hook_input = json.loads(sys.stdin.readline())
    except (json.JSONDecodeError, EOFError):
        print("No valid JSON on stdin", file=sys.stderr)
        sys.exit(0)

    transcript_path = hook_input.get("transcript_path", "")
    session_id = hook_input.get("session_id", "unknown")

    if not transcript_path:
        print("No transcript_path in hook input", file=sys.stderr)
        sys.exit(0)

    # Read and process the transcript
    transcript = read_transcript(transcript_path)
    if not transcript:
        print("Empty transcript", file=sys.stderr)
        sys.exit(0)

    conversation = extract_conversation(transcript)
    tool_calls = extract_tool_calls(transcript)
    summary = build_summary(conversation, tool_calls)

    # Save everywhere
    saved_path = save_transcript_file(transcript, session_id)
    conv_id = save_to_conversations_api(conversation, session_id)
    save_to_memory(summary)

    # Report what was saved
    report = f"📋 Pre-compaction backup complete:\n"
    report += f"   Transcript: {saved_path}\n"
    report += f"   Messages: {summary['total_turns']} turns\n"
    report += f"   Tools used: {', '.join(summary['tool_calls']) or 'none'}\n"
    if conv_id:
        report += f"   Conversation ID: {conv_id}\n"
    report += f"   Memory: session:compact-summary, session:working-on\n"
    report += f"\n   After compaction, use:\n"
    report += f"     /recall session:working-on — what you were doing\n"
    report += f"     /recall session:compact-summary — full session summary\n"

    print(report, file=sys.stderr)
    sys.exit(0)

if __name__ == "__main__":
    main()
