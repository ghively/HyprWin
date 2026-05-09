# Final Verification & Enhancement Plan

## Goal
- 100% spec compliance verification (zero gaps)
- Full Developers Guide
- AI STOP marks in every file for continued development
- Bug-free final delivery

## Stages

### Stage 1: Comprehensive Spec Audit
Read the full spec file. Compare every requirement against implementation.
Produce a detailed checklist with PASS/FAIL for every item.

### Stage 2: Bug Hunt
Multiple agents scan for:
- Logic errors (off-by-one, boundary conditions, null checks)
- Resource leaks (no CloseHandle equivalents, no thread cleanup)
- Race conditions (static mut usage, channel disconnect)
- Missing error handling (unwrap, expect, unwrap_err)
- Inconsistent behavior (focus tracking, workspace switching edge cases)

### Stage 3: Developers Guide
Create `docs/DEVELOPERS_GUIDE.md` with:
- Architecture overview with module interaction diagram
- How to add a new layout algorithm
- How to add a new IPC command
- How to add a new hotkey action
- How window lifecycle works (diagram)
- How the event pipeline works
- Common development tasks (debugging, testing)
- Build system guide
- Windows API patterns used
- Contributing guidelines

### Stage 4: AI STOP Marks
In every .rs file, at every major decision point:
- `// AI_AGENT_STOP: DECISION POINT — [description]`
- Before each match arm dispatch
- Before each algorithm choice
- At configuration boundary points
- At extensibility points

In every folder: `.ai_stop.md` describing what an AI agent should know before modifying files in that folder.

### Stage 5: Final Assembly & Commit
