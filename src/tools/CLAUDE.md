# src/tools/CLAUDE.md

Notes specific to the tools module.

## Tool Annotations

Spec: https://modelcontextprotocol.io/specification/2025-11-25/server/tools

Every `#[tool]` attribute includes a `title` (human-readable display name) and `annotations` following the MCP 2025-03-26 spec. These hints let MCP clients decide whether to confirm actions, allow autonomous execution, or flag risky operations.

```rust
#[tool(
    title = "Human Readable Name",
    annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
)]
```

### Annotation values used in this module

| Annotation | Meaning |
|---|---|
| `read_only_hint = true` | Tool does not modify any state |
| `idempotent_hint = true` | Calling multiple times with the same args has the same effect as calling once |
| `destructive_hint = true` | Tool may delete or irreversibly modify data |
| `destructive_hint = false` | Tool mutates state but is non-destructive (can be undone) |
| `open_world_hint = false` | Tool only interacts with a known bounded system (this Audiobookshelf instance), not the open internet |

### Per-tool annotation rationale

| Tool | Annotations | Reason |
|---|---|---|
| All read-only tools | `read_only_hint=true, idempotent=true, open_world=false` | Pure reads from Audiobookshelf |
| `update_progress` | `destructive=false, idempotent=true, open_world=false` | Overwrites a position, but nothing is deleted and re-sending the same value is safe |
| `create_bookmark` | `destructive=false, open_world=false` | Adds data, does not destroy anything |
| `delete_bookmark` | `destructive=true, idempotent=true, open_world=false` | Permanently removes a bookmark; calling twice on the same time value is a no-op |
| `quick_match_item` | `destructive=true, open_world=false` | Mutates metadata/covers and may overwrite existing details |

Metadata mutation payloads mirror Audiobookshelf controllers: single-item match uses top-level `overrideCover` and `overrideDetails`.
