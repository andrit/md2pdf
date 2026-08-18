Run the project's test suite and report results.

Usage: /test
       /test pytest -v --tb=short
       /test npm run test:unit

Steps:
1. Use the project_test MCP tool
2. If no command specified, auto-detects: npm test, pytest, cargo test, go test, make test
3. The tool runs the command in the project directory with a 120s timeout
4. Returns: status (passed/failed/error/timeout), output, duration
5. If tests fail: show the failure output so you can fix the issues

To set a default test command for this project:
  Update project config via API: PATCH /projects/<name>/config with {"test_command": "your command"}

Examples:
  /test                          → auto-detect and run
  /test npm run test:integration → explicit command
  /test pytest tests/ -x         → stop on first failure
