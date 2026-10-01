# TODO

Working task list for **IRONSIGHT**. Read this at the start of a work session and keep it current as work completes - check items off with a date, add follow-ups as they surface. Stale TODOs are worse than none. Security debt (if any) is tracked separately in `SECURITY-DEBT.md`.

---

## Refactor audit (2026-10-01) - found, not started

Read-only fleet audit (9 agents, nothing changed). Each line: effort S (<half day) / M (1-3 days) / L, and the risk of making the fix. 🔴 = a live bug or safety hole.

- [ ] 🟡 API response types defined twice (routes + panels) -> `src/lib/types/`; split `src/components/map/ConflictMap.tsx` (1,038, 18 `useEffect`) into one hook per feed layer. M, low-med.

## Open

_None tracked yet - add items as `- [ ] task`, grouped by priority or theme. Mark done inline: `- [x] ~~task~~ ✅ done YYYY-MM-DD`._

- [ ] Remove the leftover graphify tooling: `.claude/skills/graphify/` (10 files) + the two graphify PreToolUse hooks in `.claude/settings.json` (fleet rule: engineering-standards/repo-and-project-structure.md, "No knowledge-graph tooling"). The CLAUDE.md stub went 2026-09-30.
- [ ] `npm run lint` is `next lint`, which Next 16 removed - switch to `eslint .` with a flat config.
