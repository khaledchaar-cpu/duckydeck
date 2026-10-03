---
name: session-wrapup
description: End-of-session routine for DuckyDeck. Use when a milestone step is finished, the user says the session is done/"wrap up"/"Feierabend", or before suggesting /clear. Runs the full check, updates PROGRESS.md and proposes a commit.
---

# Session Wrap-up

Run these steps in order. Keep output short.

1. **Full check:** `scripts/check.sh` (whole workspace). If it fails, fix the problems or list them as open issues in step 2 – never report success on a red check.

2. **Update `PROGRESS.md`** (keep it at ~20 lines, overwrite outdated content instead of appending history):
   - `Aktueller Milestone` – current or next milestone from SPEC.md
   - `Erledigt` – only the last session's results, max. 5 bullets (older items: drop, git has the history)
   - `Nächster Schritt` – one concrete, immediately actionable task
   - `Offene Probleme / Notizen` – failing checks, decisions waiting for the user, findings that are not in the spec
   - If a milestone is finished that has a skill planned in PROGRESS.md (`Geplante Skills`), remind the user to create it.

3. **Spec drift:** If a decision during the session changed behaviour described in `docs/spec/*.md`, update that file (only the affected lines). If new Omarchy routes/APIs were used, make sure `scripts/gen-omarchy-reference.sh` covers them.

4. **Commit proposal:** show `git status --short` and a one-line English commit message (conventional style, e.g. `feat(daemon): hotplug detection`). Commit only if the user agrees.

5. **Final message to the user (German, max. 5 lines):** what was done, check result, next step, and that `/clear` is now safe.
