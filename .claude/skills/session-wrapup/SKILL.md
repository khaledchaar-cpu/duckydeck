---
name: session-wrapup
description: End-of-session routine for DuckyDeck. Use when a milestone step is finished, the user says the session is done/"wrap up"/"Feierabend", or before suggesting /clear. Runs the full check, updates PROGRESS.md and proposes commits.
---

# Session Wrap-up

Run these steps in order. Keep output short.

1. **Full check:** `scripts/check.sh` (whole workspace). If it fails, fix the problems or list them as open issues in step 2 – never report success on a red check.

2. **Update `PROGRESS.md`** (local, gitignored – never commit it; keep the sections, overwrite outdated content):
   - `Stand` – current state, incl. the last session's results (max. 5 bullets)
   - `Nächste Schritte` – concrete, immediately actionable tasks
   - `Offene Punkte` – failing checks, decisions waiting for the user, open findings
   - `Log` – add one dated line (`- YYYY-MM-DD: …`, German)
   - Technical pitfalls go to `CLAUDE.md` → `Stolpersteine`, not into PROGRESS.md.
   - If the project status changed, update it in `~/Projekte.md`.

3. **Spec drift:** New decisions go to `SPEC.md` → `Entscheidungen` (dated, with reason). If a decision changed behaviour described in `docs/spec/*.md`, update that file (only the affected lines). If new Omarchy routes/APIs were used, make sure `scripts/gen-omarchy-reference.sh` covers them.

4. **Commit proposal:** show `git status --short` and a one-line English commit message (conventional style, e.g. `feat(daemon): hotplug detection`). Commit only if the user agrees.

5. **Final message to the user (German, max. 5 lines):** what was done, check result, next step, and that `/clear` is now safe.
