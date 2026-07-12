# .knowledge — repo knowledge graph

One markdown file per entity in `entities/`. `id = <type>-<name>`. Frontmatter
`anchors:` = real repo paths the entity describes. Relation lines under
`## Relations` are `- <rel>: <target-id>` (vocab: depends_on, calls, stores_in,
decided_by, gotcha, part_of).

**Query:** `scripts/kg.sh list|show <id>|links <id>|backlinks <id>|anchored <path>|stale`.
Or grep: `grep -l 'demo_scan' entities/*.md`.

**Read protocol** (task start): before reading code, `kg.sh anchored <file>` for
files in scope; read those entities + their 1-hop neighbors first.

**Write protocol** (same commit as code): touched code an entity anchors and the
description/relations changed → update it. Non-obvious choice → add `decision-*`.
Lost >30 min to a surprise → add `gotcha-*` (highest ROI). Run `kg.sh stale`
before commit so renames don't rot anchors.
