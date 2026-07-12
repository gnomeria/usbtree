# Session memory: the episodic layer

The `.knowledge/` graph is **semantic** memory — curated facts, hand-promoted. This reference covers **episodic** memory: what happened in past sessions, captured automatically. The third kind, **procedural** memory (how to do things), is the skills catalog itself. The three-layer split comes from the agent-memory literature (see prior art below); the practical payoff is the promotion pipeline: episodic is the *inbox*, the knowledge graph is *long-term storage*, and skills are *habits*.

## Prior art — what the field converged on

Distilled from mem0, claude-mem, Letta/MemGPT, Zep/Graphiti, HippoRAG (index: github.com/TeleAI-UAGI/Awesome-Agent-Memory):

1. **Capture at lifecycle hooks, not by asking the model to remember.** claude-mem hooks Claude Code's `SessionStart` / `UserPromptSubmit` / `PostToolUse` / `SessionEnd` and records observations as a side effect. Anything that relies on the model volunteering "I should save this" loses data.
2. **Store compressed observations, never transcripts.** Summarize tool results and decisions at capture/session-end; raw transcripts are token landfill.
3. **Two-phase writes.** mem0's classic pipeline: extract candidate facts, then an explicit decision step — ADD / UPDATE / DELETE / NOOP against existing memories. Notably, mem0 v3 retreated to single-pass ADD-only (append, consolidate later) — at scale, simple append + offline consolidation beat clever in-line updating. Copy the v3 lesson: **append fast, consolidate in batch**.
4. **Hybrid retrieval, fused.** Keyword/BM25 + semantic embeddings + entity/graph links scored in parallel, plus recency/temporal weighting ("what's true *now*" vs "what happened *then*").
5. **Progressive disclosure.** claude-mem's three-step retrieval: `search` returns a compact index (ids + one-liners, ~50–100 tokens each) → `timeline` gives chronological context → fetch full observations only for chosen ids. ~10× token savings vs dumping results. Design every retrieval surface this way.
6. **Scoping.** Memories keyed by project/repo, session, and (for multi-user setups) user — retrieval always filters by scope first.

What **not** to copy: server dependencies (claude-mem runs a Bun HTTP worker on a port; mem0 wants a vector DB). For repo-scoped memory a single file + single binary wins — every agent can call it, nothing to daemonize, nothing to install with pip.

## Blueprint: a fast, agent-agnostic session memory (Go)

> **Reference implementation**: `tools/mem/` in this repository — a working, tested single binary implementing this blueprint (SQLite + FTS5 with porter stemming, lifecycle hooks, progressive-disclosure retrieval, `promote` into `.knowledge/`). Its README covers hook wiring per agent.

Go over Rust/Zig here, honestly assessed: the hot path is SQLite and an LLM call — the store is never the bottleneck, so systems-language wins are marginal; Go gets you a static cross-platform binary with a mature pure-Go SQLite (`modernc.org/sqlite`, no cgo) and the fastest path to done. Rust (`rusqlite`) is a fine second. Zig's SQLite story is still bring-your-own-bindings. Follow the `go-service` skill's conventions for the implementation.

### Storage: one SQLite file, `.memory/sessions.db` (WAL mode, gitignored)

```sql
CREATE TABLE sessions (
  id         TEXT PRIMARY KEY,      -- ulid
  project    TEXT NOT NULL,         -- repo root basename or explicit scope
  started_at INTEGER NOT NULL,
  ended_at   INTEGER,
  summary    TEXT                   -- written at session end
);
CREATE TABLE observations (
  id         TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  at         INTEGER NOT NULL,
  kind       TEXT NOT NULL,         -- decision|discovery|gotcha|change|todo
  body       TEXT NOT NULL,         -- 1-3 compressed sentences, never raw output
  files      TEXT NOT NULL DEFAULT '[]',  -- JSON array of touched paths
  entities   TEXT NOT NULL DEFAULT '[]'   -- JSON array of .knowledge entity ids
);
CREATE VIRTUAL TABLE obs_fts USING fts5(body, content=observations, content_rowid=rowid);
```

`entities` is the bridge to the semantic layer: observations link to `.knowledge/` ids, so "everything ever learned about service-orders" is one join. BM25 via FTS5 covers retrieval at repo scale — **skip embeddings in v1**; keyword + entity links + recency outperform vectors on codebase-shaped queries, and `sqlite-vec` can be added later without schema surgery.

### CLI surface (single binary, stdin/stdout, no daemon)

```
mem hook session-start   # reads hook JSON on stdin → prints context to inject
mem hook post-tool       # reads hook JSON → appends observation (append-only, fast)
mem hook session-end     # summarizes session's observations → sessions.summary
mem search <query>       # compact index: id | date | kind | one-liner  (BM25+recency)
mem timeline <obs-id>    # neighbors in time around an observation
mem show <obs-id>...     # full bodies, only for chosen ids
mem consolidate          # batch: dedupe exact repeats, flag promote candidates
mem promote <obs-id>     # create/update a .knowledge/ entity from an observation
```

`search`/`timeline`/`show` is the progressive-disclosure triple. `promote` closes the loop with the knowledge graph: a gotcha observed twice is one `mem promote` away from becoming a permanent `gotcha-*` entity that the *read protocol* surfaces forever.

### Hook wiring (per agent)

- **Claude Code**: `SessionStart`, `PostToolUse`, `SessionEnd` hooks in settings; each pipes its JSON event to `mem hook <name>`. `SessionStart` output (the injected context) = current project's last few session summaries + observations anchored to files in scope — filtered through progressive disclosure, a few hundred tokens max.
- **Gemini CLI / opencode**: equivalent hook/plugin events where offered.
- **Any headless agent** (the `orchestrate` skill's workers): no hooks needed — the brief already mandates a results report; run `mem hook session-end` over the worker's log/report in the wrapper, or skip capture and rely on results files.
- **No hooks at all**: `mem search`/`mem show` still work as plain CLI tools any agent can call when its instructions say "check memory first".

### Consolidation policy (the part most systems get wrong)

- Append-only during sessions; `mem consolidate` runs at session-end or cron: exact-duplicate collapse, repeated discoveries flagged for `promote`. (Todo decay/merging: blueprint-only, not implemented in `tools/mem` yet.)
- Episodic memory is allowed to be messy and is **gitignored**; only promoted knowledge enters `.knowledge/` and code review. That boundary keeps the curated graph trustworthy while capture stays cheap.

## Layering summary

| Layer | Lives in | Written by | Trust |
|---|---|---|---|
| Episodic — sessions, observations | `.memory/sessions.db` (gitignored) | hooks, automatically | raw, dedupe on read |
| Semantic — entities, relations | `.knowledge/` (committed) | agents/humans, deliberately (`promote`) | reviewed in PRs |
| Procedural — how-to | skills catalog | humans | versioned |
