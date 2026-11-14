# 0042: Shared native content validation

Status: accepted for the supported spell, tool-part, tool-material and reaction catalogs.

Forge and command-line authors publish through the existing revision-checked
workspace/bundle transaction. Native checker examples call the same typed parsers
as runtime loading. Their receipt binds binaries to native sources and the packed
material table. Missing or stale checkers block dependent catalog publication.
Persisted tool material, part and schema row identities remain stable.

The content crate holds the generic spell schema; audio types are supplied by its
caller. Add engine_audio as an engine_content development dependency so its spell
checker validates the real Cue enum without coupling the content library to audio.
Add serde_json as a development dependency of engine_content and engine_tools for
the checker protocol. It already exists in the workspace lockfile; runtime crates
do not acquire this dependency.

The initial alloy workflow edits an existing tool material and its physical molten
batch reaction. The bronze kiln derives its ingredient and output counts from that
same reaction via existing smeltery resource mappings. The command does not mint
block/resource IDs, add simulation primitives, or hot-reload compiled content.

Human System Boards and AI commands use the same native validation, revision
checks, backups and bundle reversion. Cross-catalog compilation for new identities
and a running-game reload protocol remain future native work.
