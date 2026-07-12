# Formats: file graph, relation vocabulary, SQLite tier

## Entity file (Tier 1)

`.knowledge/entities/<id>.md`, where `id = <type>-<name>` in kebab-case. Machine-parseable frontmatter, short prose, typed relation lines:

```markdown
---
id: service-orders
type: service
anchors: [internal/orders/, api/openapi.yaml]
---
# Orders service

Owns order lifecycle (create → pay → fulfill). State machine lives in
service.go; handlers are thin. Money is integer cents everywhere — floats
rejected in review, see decision-money-cents.

## Relations
- depends_on: service-users
- stores_in: store-postgres
- gotcha: gotcha-order-status-race
```

Rules:

- **anchors** are real repo paths (dirs or files). They're the join key between graph and code: "which entities cover the files I'm about to change" is `grep -l 'internal/orders' .knowledge/entities/*.md`. `kg.sh stale` flags anchors whose paths no longer exist.
- **Relation lines** are exactly `- <relation>: <target-id>` under `## Relations` — one edge per line, greppable, nothing else in that section.
- Prose earns its place only if the code can't say it faster: purpose, invariants, the *why* behind the shape.

## Relation vocabulary

Keep to ~8 relations; a vocabulary nobody remembers doesn't get queried.

| Relation | Meaning |
|---|---|
| `depends_on` | needs the target to function (code or runtime) |
| `calls` | invokes the target's API at runtime |
| `stores_in` | persists data in the target store |
| `emits` / `consumes` | event/queue producer and consumer |
| `decided_by` | shape explained by a `decision-*` entity |
| `gotcha` | trap documented in a `gotcha-*` entity |
| `part_of` | belongs to a larger `flow-*` or system entity |

Add a new relation only when an existing one is genuinely wrong, and note it in `.knowledge/README.md`.

## Decision and gotcha entities

```markdown
---
id: decision-auth-jwt
type: decision
anchors: [internal/auth/]
---
# Auth: stateless JWT over sessions

Chose JWT (15-min access + rotating refresh) because the API is consumed by
mobile clients with no cookie jar. Rejected: server sessions (needs sticky
Redis), OAuth-only (still need first-party auth).

## Relations
- part_of: service-users
```

```markdown
---
id: gotcha-sqlite-wal
type: gotcha
anchors: [internal/store/db.go]
---
# SQLite WAL mode required

Without `PRAGMA journal_mode=WAL` the test suite deadlocks under parallel
writers — cost a full afternoon. It's set in db.go init; don't remove it.

## Relations
- gotcha: store-sqlite
```

## Tier 2 — SQLite

Escalate when the file graph passes ~200 entities or you need traversal (transitive impact, paths). Single file `.knowledge/graph.db`, queried with the `sqlite3` CLI every agent can run:

```sql
CREATE TABLE nodes (
  id      TEXT PRIMARY KEY,          -- service-orders
  type    TEXT NOT NULL,             -- service|store|decision|gotcha|flow
  body    TEXT NOT NULL,             -- the prose
  anchors TEXT NOT NULL DEFAULT '[]' -- JSON array of paths
);
CREATE TABLE edges (
  src TEXT NOT NULL REFERENCES nodes(id),
  rel TEXT NOT NULL,
  dst TEXT NOT NULL REFERENCES nodes(id),
  PRIMARY KEY (src, rel, dst)
);
CREATE INDEX edges_dst ON edges(dst);
```

Migration is mechanical — parse each entity file's frontmatter and relation lines into `nodes`/`edges` rows. Keep the markdown as the human-editable source and regenerate the DB, or cut over entirely; don't maintain both by hand.

Useful queries:

```sql
-- reverse edges: what breaks if service-users changes?
SELECT src, rel FROM edges WHERE dst = 'service-users';

-- transitive impact (everything downstream of a node)
WITH RECURSIVE impact(id) AS (
  SELECT 'service-users'
  UNION
  SELECT e.src FROM edges e JOIN impact i ON e.dst = i.id
)
SELECT id FROM impact WHERE id != 'service-users';

-- entities anchored to a path I'm changing
SELECT id FROM nodes, json_each(nodes.anchors)
WHERE json_each.value LIKE 'internal/orders%';
```

## Tier 3 — when a real graph DB is actually right

Only when the **application** ships graph features (recommendations, dependency analysis as a product). Then it's an app dependency like any other — model it with the `sql-schema` skill's discipline and keep agent memory in Tier 1/2 regardless: agent memory must stay readable by every agent and reviewable in diffs, which a DB server never is.
