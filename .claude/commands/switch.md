Switch active workbench project — or register/create a project.

Shows all registered projects and lets you switch to one, register an existing
project directory, or create a new project from a template.

## Steps

### 1 — Get the project list

Call `GET http://mcp-server:3100/projects` and parse the response.
Also read the currently active project: `grep "^WORKBENCH_PROJECT=" /workbench/.env | cut -d= -f2-`

Display a clean summary so the user can see all options before choosing:
```
Registered projects:
  * factory        (factory)     — active
    syntaxpress    (ecommerce)
    bag            (fullstack)
    workbench      (fullstack)
```

### 2 — Ask what to do

Use AskUserQuestion with these options:
- "Switch to a registered project" — select from the list above
- "Register an existing project" — add a project directory already on disk
- "Create a new project" — scaffold from a project type template

### 3a — Switch to registered project

Ask: "Which project?" (user types the name — they just saw the list).

Read the project from the API: `GET http://mcp-server:3100/projects/<name>`

If not found, say so and stop.

Output exactly:
```
To switch, run this in your host terminal:

  make activate NAME=<name>

The container will restart and Claude Code will reopen automatically.
```

### 3b — Register an existing project

Gather via AskUserQuestion (or sequential questions if more than 4 fields):
- **Project name** — slug, no spaces (e.g. `syntaxpress`)
- **Project type** — select from: fullstack, ecommerce, mobile, pwa, cli, rag, agent,
  multi-agent, data-pipeline, iot, factory, microservices, api-integration, custom
- **Full directory path** — absolute path on the host (e.g. `/Users/you/myproject`)

Call `POST http://mcp-server:3100/projects` with body:
```json
{ "name": "<name>", "type": "<type>", "directory": "<directory>" }
```

On success, output:
```
✓ Registered: <name> (<type>) at <directory>

To switch to it now, run in your host terminal:

  make activate NAME=<name>
```

On 409 (already registered), say so and offer to switch to it instead.

### 3c — Create a new project

Read PROJECTS_DIR: `grep "^PROJECTS_DIR=" /workbench/.env | cut -d= -f2-`

If PROJECTS_DIR is empty or still the placeholder `/path/to/your/projects`, tell the user:
"Set PROJECTS_DIR in your .env file first — it's the parent directory where new projects will be created."
Then stop.

Gather via AskUserQuestion:
- **Project name** — slug, no spaces (e.g. `nexus`)
- **Project type** — select from the 14 types (list them)
- **Directory name** — folder name to create (default: same as project name)
- **Description** — one-liner for CLAUDE.md (optional, can be empty)

Compute the full path: `<PROJECTS_DIR>/<directory-name>`

Output exactly (this runs on the host, not inside the container):
```
To create and activate this project, run these two commands in your host terminal:

  make scaffold NAME=<name> TYPE=<type> DIR=<full-path>
  make activate NAME=<name>

What these do:
  make scaffold  — creates the directory, writes CLAUDE.md and documents/ from
                   the <type> template, registers the project in the workbench
  make activate  — restarts claude-code with the new project mounted, then
                   drops you straight into Claude Code
```

If a description was provided, add:
```
After scaffolding, open CLAUDE.md and add this to the description section:
  <description>
```

## Notes

- "Register existing" only adds the project to the workbench registry (database).
  It does NOT modify any files in the project directory.
- "Create new" cannot run from inside Claude Code — it writes files to an external
  directory that this container cannot reach. The host commands do this correctly.
- The make activate restart takes ~10 seconds and drops you back into Claude Code
  automatically — no manual reconnect step needed.
