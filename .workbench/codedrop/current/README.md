# Codedrop — Current Phase

Place code files, patches, and corrections here for the AI to pick up and place.
The AI processes these at session start — reads each file, confirms destination, then places it.

Optional sidecar (.meta.json alongside each file):
  { "destination": "src/components/Button.tsx", "instruction": "replace the existing file" }

Multi-file batch (.codedrop.json manifest):
  { "files": [{ "name": "Button.tsx", "destination": "src/components/", "instruction": "..." }] }

Without a sidecar the AI infers the destination and confirms before acting.
Not auto-ingested — codedrop items are processed deliberately, not fed into RAG.
