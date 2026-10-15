# Project knowledge maintenance

- Read [PROJECT_KNOWLEDGE.md](PROJECT_KNOWLEDGE.md) before exploring the codebase. Use its task-to-source index to scope further inspection.
- Keep `PROJECT_KNOWLEDGE.md` consistent with the code in the same change. Before completing work, compare the diff against the relevant document sections and update any affected behavior, contracts, ownership/lifetime rules, persistence formats, build commands, package/module status, source pointers, and known limitations.
- When fixing a documented limitation, remove or rewrite the outdated description. When adding, moving, renaming, or deleting a subsystem or entry point, update its package map and task-to-source references.
- Correct discrepancies encountered while working. Source code and manifests are authoritative; inspect the relevant implementation rather than preserving a stale claim.
- Document the current state, not change history. Keep updates scoped to affected sections; do not repeat a whole-project analysis for a local change. If the diff does not affect documented facts, no document edit is required.
- Distinguish source-inspection findings from executed validation. Record only checks actually run, with their scope and date; do not refresh the whole-document snapshot date unless the whole document was revalidated.
- Treat required knowledge-document updates as part of task completion, not optional follow-up work.

# Instructions

Do not cheat. Don't go for path of least resistance when implementing a feature.
Use engine capabilities when implementing plugins.
