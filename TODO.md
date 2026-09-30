# TODO

Working task list for **IRONSIGHT**. Read this at the start of a work session and keep it current as work completes - check items off with a date, add follow-ups as they surface. Stale TODOs are worse than none. Security debt (if any) is tracked separately in `SECURITY-DEBT.md`.

---

## Open

_None tracked yet - add items as `- [ ] task`, grouped by priority or theme. Mark done inline: `- [x] ~~task~~ ✅ done YYYY-MM-DD`._

- [ ] Remove the leftover graphify tooling: `.claude/skills/graphify/` (10 files) + the two graphify PreToolUse hooks in `.claude/settings.json` (fleet rule: engineering-standards/repo-and-project-structure.md, "No knowledge-graph tooling"). The CLAUDE.md stub went 2026-09-30.
- [ ] `npm run lint` is `next lint`, which Next 16 removed - switch to `eslint .` with a flat config.
