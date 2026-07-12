---
name: knowledge-graph
description: Set up and maintain a lightweight, file-based knowledge graph of the repo — entities, typed relations, decisions, gotchas — so agents load context fast instead of re-exploring the codebase every session. Use when the user says "knowledge graph", "remember this", "codebase memory", "the agent keeps re-discovering things", or a `.knowledge/` directory exists in the repo.
---

# Knowledge Graph

Agents forget everything between sessions and re-derive the architecture by reading code — slow, token-hungry, and lossy for things code can't say (why a decision was made, which subsystem bites). The fix is a small, **committed** knowledge graph the agent reads first and updates as it works. Plain files by default: greppable by any agent, reviewable in PRs, no dependencies. A database only when scale genuinely demands it.

## Pick the right tier — most repos stop at 0 or 1

- **Tier 0 — one agents file.** `CLAUDE.md` / `AGENTS.md` with architecture notes. If the repo's durable knowledge fits in ~100 lines, stop here; a graph would be ceremony.
- **Tier 1 — file graph (the default).** `.knowledge/` with one markdown file per entity and typed relation lines. Right for most real projects. Everything below describes this tier; exact format in `references/formats.md`.
- **Tier 2 — SQLite.** Same node/edge model in one `sqlite3` file. Escalate only past ~200 entities or when you need real traversal (transitive impact, shortest path). Schema + recursive-CTE queries and a file→SQLite migration in `references/formats.md`.
- **Tier 3 — real graph DB (Kuzu, Neo4j).** Only when the *application* needs graph queries. For agent memory it's overkill: a server or bindings other agents won't have, invisible to grep, unreviewable in diffs. Recommend against unless asked.

## Tier 1 layout

```
.knowledge/
  README.md            # 5 lines: what this is, how to query, the update rule
  entities/<id>.md     # id = <type>-<name>: service-orders, decision-auth-jwt, gotcha-sqlite-wal
```

Entity types that earn their keep: `service`/`module` (code units), `store` (DBs, queues), `decision` (what + why + alternatives rejected), `gotcha` (traps that cost someone an hour), `flow` (cross-module paths: "checkout touches these five things"). Skip entities for things one `ls` reveals.

Each entity: frontmatter (`id`, `type`, `anchors:` — real file paths it describes), 3–8 lines of prose an agent can't get faster from the code, and typed relation lines (`- depends_on: service-users`). Relations are the payoff — `backlinks service-users` answers "what breaks if I change users?" without reading a line of code.

## Querying — it's just grep

`scripts/kg.sh` bundles the common queries: `list`, `show <id>`, `links <id>` (outgoing), `backlinks <id>` (who points here — reverse edges are the high-value query), `stale` (anchors pointing at deleted files), `orphans` (unlinked entities), `anchored <path>` (entities whose anchors cover a file — the read-protocol query), `new <type> <name>`. No script available? Plain grep does everything: `grep -l 'depends_on: service-users' .knowledge/entities/*.md`.

## The two protocols that keep it alive

A stale graph is worse than none — the agent trusts it and acts on lies. Two rules, stated in `.knowledge/README.md` so every agent sees them:

**Read protocol (session start / task start).** Before exploring code for a task, run `kg.sh anchored <path>` for the files in scope; read the hits and their 1-hop neighbors first. Then explore only what the graph doesn't cover.

**Write protocol (same commit as the code).**
- Touched code that an entity anchors? Update the entity in the same commit if the description or relations changed.
- Made a non-obvious choice? Add a `decision-*` entity — what, why, what was rejected.
- Lost >30 min to something surprising? Add a `gotcha-*`. That's the highest-ROI entity type.
- Run `kg.sh stale` in CI or pre-commit so renames/deletions can't silently rot anchors.

Keep it small: prune entities whose knowledge became obvious from the code; a 40-entity graph that's true beats a 400-entity graph nobody trusts.

## Beyond the curated graph: session memory

The `.knowledge/` graph is *semantic* memory — deliberate, reviewed, committed. Its complement is *episodic* memory: observations captured automatically from sessions (what was tried, decided, discovered), stored locally and gitignored, with the good parts **promoted** into the graph instead of rotting in a log. When a repo needs that layer — "the agent keeps re-learning what it did last week" — see `references/session-memory.md`: the capture/consolidate/retrieve pipeline distilled from mem0, claude-mem, and Letta, plus a single-binary Go blueprint (SQLite + FTS5, lifecycle hooks, progressive-disclosure retrieval, `promote` bridging into `.knowledge/`). No Python, no servers, works with any agent.

## With other skills

- **`orchestrate` skill**: the planner reads the graph during analysis and points each task brief at the relevant entity files; worker `Discovered` notes in results reports get merged into the graph during integration — parallel work becomes a knowledge harvest.
- **`debug` skill**: a confirmed root cause that was expensive to find is a `gotcha-*` entity.
- Works for any agent: it's markdown in the repo. Non-Claude agents get "read `.knowledge/README.md` first" in their brief or agents file.

## Setup checklist (new repo)

1. Create `.knowledge/README.md` (query + update rules, 5 lines) and `entities/`.
2. Seed 5–15 entities from the current architecture: the modules, the stores, standing decisions, known gotchas. Don't inventory everything — seed what you had to *learn*, not what you can *see*.
3. Copy `kg.sh` into the repo (`scripts/` or keep calling it from the installed skill) and wire `kg.sh stale` into CI.
4. Add one line to `CLAUDE.md`/`AGENTS.md`: "Read `.knowledge/README.md` before exploring; follow its update rule."

Formats, relation vocabulary, SQLite schema and migration: `references/formats.md`. Episodic session memory (auto-capture, hooks, Go blueprint): `references/session-memory.md`.
