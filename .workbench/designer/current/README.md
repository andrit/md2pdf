# Designer Input — Current Phase

Place design notes, briefs, architectural decisions, and intent documents here.
These files are your natural language code — the higher-order programming layer.
The AI reads everything in this directory at session start, before acting.

As the project advances, archive files into phase subdirectories:
  .workbench/designer/phase-0/
  .workbench/designer/phase-1/
The `current/` directory always holds the active phase's designer input.

File types: .md, .txt, .json
Auto-ingested: yes — the file watcher picks these up into the RAG knowledgebase when WATCH_ENABLED=true.
