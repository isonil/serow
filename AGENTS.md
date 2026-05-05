# Serow Agent Instructions

This repository implements Serow, an AI-first programming language.

Before adding public behavior, use the Serow tool interface instead of relying only on file search:

1. Run `bin/serow query intent "<description>"` to check for existing functionality.
2. Run `bin/serow query symbol "<name>"` when a symbol might already exist.
3. Run `bin/serow check` after edits.
4. Run `bin/serow certify` before considering changed Serow code complete.

Current bootstrap constraints:

- The primary implementation is dependency-free Rust.
- The earlier Python bootstrap remains in `serowlang/` temporarily as reference code.
- Source programs live in `examples/` or any path passed to `bin/serow check <path>`.
- Public functions must declare `intent`, `contract`, `examples`, `properties`, `effects`, and `impl`.
- Examples are executable tests.
- Properties currently support simple sampled `forall` checks over `Int` and `Bool`.
- Generated or compiled backends do not exist yet; do not invent them without updating `Progress/`.
