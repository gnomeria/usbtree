#!/usr/bin/env bash
# kg.sh — query helpers for the .knowledge/ file graph. Run from the repo root.
#
# usage: kg.sh <command> [args]
#   list                 all entity ids
#   show <id>            print an entity
#   links <id>           outgoing relations of an entity
#   backlinks <id>       entities with an edge pointing at <id>
#   anchored <path>      entities whose anchors cover <path>
#   stale                anchors pointing at paths that no longer exist
#   orphans              entities with no edges in or out
#   new <type> <name>    create entities/<type>-<name>.md from template
set -euo pipefail

KG=${KG_DIR:-.knowledge}
ENT="$KG/entities"

die() { echo "kg.sh: $*" >&2; exit 1; }
need_graph() { [ -d "$ENT" ] || die "no $ENT/ — run 'kg.sh new' or see the knowledge-graph skill"; }
entity_file() { [ -f "$ENT/$1.md" ] || die "no entity '$1'"; echo "$ENT/$1.md"; }

cmd=${1:-help}; shift || true
case "$cmd" in
  list)
    need_graph
    for f in "$ENT"/*.md; do [ -f "$f" ] && basename "$f" .md; done | sort
    ;;
  show)
    [ $# -eq 1 ] || die "usage: kg.sh show <id>"
    cat "$(entity_file "$1")"
    ;;
  links)
    [ $# -eq 1 ] || die "usage: kg.sh links <id>"
    grep -E '^- [a-z_]+: ' "$(entity_file "$1")" || echo "(no outgoing relations)"
    ;;
  backlinks)
    [ $# -eq 1 ] || die "usage: kg.sh backlinks <id>"
    need_graph
    grep -lE "^- [a-z_]+: $1\$" "$ENT"/*.md 2>/dev/null \
      | while read -r f; do basename "$f" .md; done || echo "(no backlinks)"
    ;;
  anchored)
    [ $# -eq 1 ] || die "usage: kg.sh anchored <path>"
    need_graph
    # capture first: a bare || echo binds to the pipeline's last inner grep,
    # printing "(nothing anchored)" even after real matches
    hits=$(grep -l "$1" "$ENT"/*.md 2>/dev/null \
      | while read -r f; do grep -q '^anchors:.*'"$1" "$f" && basename "$f" .md; done) || true
    if [ -n "$hits" ]; then printf '%s\n' "$hits"; else echo "(nothing anchored to $1)"; fi
    ;;
  stale)
    need_graph
    rc=0
    for f in "$ENT"/*.md; do
      [ -f "$f" ] || continue
      id=$(basename "$f" .md)
      # anchors: [a, b, c] → one path per line
      sed -n 's/^anchors:[[:space:]]*\[\(.*\)\]/\1/p' "$f" | tr ',' '\n' \
        | sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | while read -r a; do
            [ -n "$a" ] && [ ! -e "$a" ] && echo "STALE $id: anchor '$a' does not exist"
          done | grep . && rc=1 || true
    done
    exit "$rc"
    ;;
  orphans)
    need_graph
    for f in "$ENT"/*.md; do
      [ -f "$f" ] || continue
      id=$(basename "$f" .md)
      out=$(grep -cE '^- [a-z_]+: ' "$f" || true)
      inc=$(grep -lE "^- [a-z_]+: $id\$" "$ENT"/*.md 2>/dev/null | grep -cv "^$f\$" || true)
      [ "${out:-0}" -eq 0 ] && [ "${inc:-0}" -eq 0 ] && echo "$id"
    done
    exit 0
    ;;
  new)
    [ $# -eq 2 ] || die "usage: kg.sh new <type> <name>"
    id="$1-$2"; f="$ENT/$id.md"
    [ -f "$f" ] && die "entity '$id' already exists"
    mkdir -p "$ENT"
    cat > "$f" <<EOF
---
id: $id
type: $1
anchors: []
---
# $id

TODO: 3-8 lines the code can't say faster — purpose, invariants, the why.

## Relations
EOF
    echo "created $f"
    ;;
  *)
    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
    ;;
esac
