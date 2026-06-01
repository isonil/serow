# Implementation Log

## 2026-06-01

- Chose standard-library reference cleanup because `docs/stdlib.md` still listed list property sampling as future scope after the checker, language reference, README, and progress state all documented bounded homogeneous `List<T>` sampled properties as supported.
- Updated the stdlib limits text to keep source-level generics, generic `Option<T>`, higher-order collection helpers, and external randomness in future scope without excluding existing list property samples.

- Chose Python reference parser/checker parity as a contained cleanup because the Rust bootstrap already handles escaped quotes in executable string arguments, while the temporary Python reference split example call arguments without honoring escaped string delimiters.
- Made the Python example argument splitter escape-aware inside strings and added a regression for an escaped quote followed by a comma inside a `Text` argument.

- Chose Python reference sampler parity as a low-risk cleanup because the Rust bootstrap and public docs support bounded homogeneous `List<T>` property samples, while the temporary Python checker still treated every list binding as non-executable.
- Added Python list property samples matching the Rust empty/singleton/two-element-prefix shape, made sample formatting and shrinking complexity list-aware, and fixed direct-call argument splitting for list literals containing commas.

- Chose public progress documentation cleanup because the canonical project/crate version and CLI/backend docs already describe `1.0.28` list property sampling, while the README, roadmap, and current-state closure summary still implied the older `1.0.27` signed-zero milestone or omitted homogeneous list samples from the sampled-property overview.
- Updated those public summaries so future agent iterations choose work from the current state instead of stale progress notes.

## 2026-05-31

- Chose deterministic list property sampling as a small completeness fix because `List<T>` values already check, evaluate, lower to IR, and compile to Rust, but `forall` bindings over list values were still reported as non-executable.
- Added bounded list samples built from supported element samples, emitted typed Rust property-test bindings so empty sampled lists compile reliably, updated docs/progress metadata, and bumped Serow to `1.0.28-rust-bootstrap` / crate `1.0.28`.
- Verification is recorded in the final run for this change.

- Chose Python reference evaluator parity as a low-risk cleanup because the Rust interpreter now has explicit Serow value equality for Float, list, and record comparisons, while the temporary Python bootstrap still delegated equality and list membership/removal to raw Python comparisons.
- Added a shared Python Serow value-equality helper for comparisons plus `contains`/`remove_first`, and covered signed-zero Float equality through scalar, list, record, membership, and removal evidence in the Python regression suite.
- Verification is recorded in the final run for this change.

- Chose signed-zero Float equality as a correctness fix because the checker interpreter compared Float values by stored bits, so `-0.0 == 0.0` failed during Serow evidence checks even though generated Rust `f64` equality treats them as equal.
- Updated interpreter equality to compare Float values numerically and recurse through records as well as lists, covered signed-zero equality for scalar, list, and record values, and bumped Serow to `1.0.27-rust-bootstrap` / crate `1.0.27`.
- Verification is recorded in the final run for this change.

- Chose safe float-list access as a small bootstrap completeness fix because finite `List<Float>` values already support the other list intrinsics, but the temporary non-generic safe access intrinsics skipped floats.
- Added compiler-owned `get_float(list: List<Float>, index: Int) -> MaybeFloat` across intrinsic discovery, type checking, evaluation, IR/Rust backend emission, Python bootstrap parity, the stdlib list helpers, docs, and regression coverage. Missing indexes return `{ found: false, value: 0.0 }`, matching the existing non-panicking access contract.
- Bumped Serow to `1.0.26-rust-bootstrap` / crate `1.0.26`.
- Verification is recorded in the final run for this change.

- Chose Python reference test maintenance as a low-risk cleanup because the Rust and Python bootstraps now both check the current examples cleanly, but the Python aggregate smoke test still expected older fixture totals.
- Updated the Python bootstrap example-count regression to the then-current canonical `serow check` totals: 97 functions, 226 examples, and 97 properties.
- Verification is recorded in the final run for this change.

- Chose language-reference cleanup after a checked stdlib experiment exposed an undocumented bootstrap constraint: declared type names are global across the checked source set rather than module-scoped.
- Documented the v1 uniqueness rule in `docs/language.md` so future stdlib and example work can avoid cross-module record-name collisions.

- Chose Cargo manifest version parser hardening as a small release-readiness fix because the dependency-free reader used by release checks only accepted an exact `[package]` header, while valid TOML may include header whitespace and trailing comments.
- Updated the parser to accept spaced/commented `[package]` table headers while rejecting malformed trailing table text, covered the case in the manifest parser regression, and bumped Serow to `1.0.25-rust-bootstrap` / crate `1.0.25`.
- Verification is recorded in the final run for this change.

- Chose project-manifest parser hardening as a small public v1 correctness fix because the release/version metadata reader accepted a valid quoted prefix while ignoring malformed trailing content before the comma or closing brace.
- Tightened top-level project string parsing to require the quoted string to consume the full JSON value slice, covered the malformed trailing-version case, and bumped Serow to `1.0.24-rust-bootstrap` / crate `1.0.24`.
- Verification is recorded in the final run for this change.

- Chose safe bool-list access as a small bootstrap completeness fix because Serow supports homogeneous `List<Bool>` values and list helpers, but the temporary non-generic safe access intrinsics only covered `Text` and `Int`.
- Added compiler-owned `get_bool(list: List<Bool>, index: Int) -> MaybeBool` across intrinsic discovery, type checking, evaluation, IR/Rust backend emission, the stdlib list helpers, docs, and regression coverage. Missing indexes return `{ found: false, value: false }`, matching the existing non-panicking access contract.
- Bumped Serow to `1.0.23-rust-bootstrap` / crate `1.0.23`.
- Verification is recorded in the final run for this change.

- Chose docs command ergonomics as a small public v1 hardening task because `bin/serow docs --check` was the documented automation spelling, while the intuitive `bin/serow docs check` form failed as a positional-argument usage error.
- Added `docs check` as a narrow alias for `docs --check` without changing `--` separator behavior, updated command discovery/docs/progress metadata, covered the alias with a focused regression, and bumped Serow to `1.0.22-rust-bootstrap` / crate `1.0.22`.
- Verification is recorded in the final run for this change.

## 2026-05-25

- Chose fenced-code heading-anchor handling as a public v1 release-hardening task because `docs --check` ignored link syntax inside fenced code blocks, but still treated heading-like lines inside those blocks as real Markdown anchors.
- Updated Markdown anchor collection to skip fenced code blocks, covered the anchor false-positive with a focused regression, and bumped Serow to `1.0.21-rust-bootstrap` / crate `1.0.21`.
- Verification is recorded in the final run for this change.

- Chose inline-code reference definition handling as a public v1 release-hardening task because `docs --check` ignored link usages and targets inside inline code spans, but reference-style definitions inside inline code could still satisfy real full/collapsed reference usages.
- Updated reference-label collection to strip inline code spans before accepting Markdown reference definitions, covered the missing-definition bypass with a focused regression, and bumped Serow to `1.0.20-rust-bootstrap` / crate `1.0.20`.
- Verification is recorded in the final run for this change.

- Chose reference-style Markdown usage validation as a public v1 release-hardening task because `docs --check` validated inline links and reference definition targets, but full/collapsed reference usages could still point at missing definitions.
- Added missing-definition detection for full and collapsed reference-style Markdown link usages outside code spans/fences, covered valid and broken usages with a focused regression, and bumped Serow to `1.0.19-rust-bootstrap` / crate `1.0.19`.
- Verification is recorded in the final run for this change.

- Chose titled inline Markdown link validation as a public v1 release-hardening task because `docs --check` validated inline local Markdown links and reference-style links with titles, but valid inline links with optional titles were treated as paths containing the title text.
- Updated inline Markdown link destination parsing to ignore optional titles, including angle-bracket destinations, covered valid and broken titled-link cases with a focused regression, and bumped Serow to `1.0.18-rust-bootstrap` / crate `1.0.18`.
- Verification is recorded in the final run for this change.

- Chose reference-style documentation link validation as a public v1 release-hardening task because `docs --check` validated inline local Markdown links and anchors, but reference-style link definitions could still point at missing local files or headings.
- Extended the docs link scanner to validate reference-style local Markdown link definitions, including angle-bracket destinations with titles, covered missing-file and missing-anchor cases with a focused regression, and bumped Serow to `1.0.17-rust-bootstrap` / crate `1.0.17`.
- Verification is recorded in the final run for this change.

- Chose documentation link scanning as a public v1 release-hardening task because `docs --check` validated public Markdown links but still treated sample Markdown link syntax inside code examples as real links.
- Updated the docs link scanner to skip fenced code blocks and inline backtick code spans before validating local links and heading anchors, covered the behavior with a focused regression, and bumped Serow to `1.0.16-rust-bootstrap` / crate `1.0.16`.
- Verification is recorded in the final run for this change.

- Chose project-overview documentation discovery as a public v1 release-hardening task because `docs --check` already validated README links, but `bin/serow docs` did not advertise `README.md` as a stable local reference.
- Added the README project overview to `bin/serow docs` text/JSON reference rows, updated CLI/progress metadata, and covered the JSON listing with a focused regression.
- Bumped Serow to `1.0.15-rust-bootstrap` / crate `1.0.15`; verification is recorded in the final run for this change.

- Chose release metadata consistency as a public v1 release-hardening task because `release-check` aggregated the Serow-owned gates but did not prove the canonical `serow.project` version matched the Rust crate version in `Cargo.toml`.
- Added a dependency-free Cargo package version reader, made `release-check` report and gate `release_metadata` with the expected `-rust-bootstrap` project version, covered the mismatch case with an isolated integration fixture, and updated CLI docs/progress metadata.
- Bumped Serow to `1.0.14-rust-bootstrap` / crate `1.0.14`; verification is recorded in the final run for this change.

- Chose documentation anchor validation as a public v1 release-hardening task because `docs --check` and `release-check` validated local documentation files but still accepted local Markdown links whose `#heading` fragments did not resolve.
- Extended local Markdown link validation to check same-file and cross-file Markdown heading anchors, kept JSON `broken_links` output source-line oriented, covered valid and broken anchor cases with an integration regression, and updated CLI docs/progress metadata.
- Bumped Serow to `1.0.13-rust-bootstrap` / crate `1.0.13`; verification is recorded in the final run for this change.

- Chose discovery-command separator handling as a public v1 protocol-hardening task because `help`, `version`, path-taking commands, and structured patch commands already treated `--json` after `--` as literal input, while `agent` and `docs` still detected it as an output flag.
- Updated `agent` and `docs` to honor `--json` only before an argument separator, kept `docs --check` under the same separator rule, and covered the separated `--json` cases with focused CLI regressions.
- Bumped Serow to `1.0.12-rust-bootstrap` / crate `1.0.12`; verification is recorded in the final run for this change.

- Chose structured patch JSON separator handling as a public v1 patch-hardening task because path-taking commands already respected `--` for literal dash-prefixed values, while patch commands stripped `--json` anywhere in the argument list and could not pass it as metadata.
- Updated patch command dispatch and subcommand parsing so JSON output flags are honored only before `--`, literal post-separator values are preserved, the inherited JSON flag is inserted before the subcommand separator, and CLI docs describe the separator behavior for structured patch arguments.
- Bumped Serow to `1.0.11-rust-bootstrap` / crate `1.0.11`; verification is recorded in the final run for this change.

- Chose top-level option usage classification as a public v1 patch-hardening task because nested command families already classify `--...` values as unknown options, while the top-level dispatcher still described option-looking command slots as unknown commands.
- Updated the top-level dispatcher to report `Unknown serow option` for dash-prefixed unknown command slots, covered both ordinary and leading-`--json` JSON diagnostics, and bumped Serow to `1.0.10-rust-bootstrap` / crate `1.0.10`.
- Verification is recorded in the final run for this change.

## 2026-05-24

- Chose nested command-family usage classification as a public v1 patch-hardening task because `agent`, `compile`, `query`, `replay`, and `patch` returned structured usage diagnostics, but option-looking values in the subcommand/target slot were still described as unknown subcommands.
- Updated those command-family dispatchers to classify `--...` values as unknown options while preserving unknown subcommand/target wording for ordinary words, covered the JSON protocol with focused regressions, and bumped Serow to `1.0.9-rust-bootstrap` / crate `1.0.9`.
- Verification is recorded in the final run for this change.

- Chose public documentation link validation as a v1 release-hardening task because `docs --check` verified advertised files existed but did not catch broken local Markdown links inside the public docs set.
- Extended `bin/serow docs --check` and `bin/serow release-check` to validate local Markdown links, added JSON `markdown_links_ok` and `broken_links` fields, covered the failing-link case with an integration regression, and bumped Serow to `1.0.8-rust-bootstrap` / crate `1.0.8`.
- Verification is recorded in the final run for this change.

- Chose discovery-command usage classification as a public v1 patch-hardening task because `docs` and `version` returned structured usage diagnostics, but unknown option-looking arguments were still described as positional extras unlike neighboring discovery commands.
- Updated `docs` and `version` usage handling to report unknown options explicitly while preserving positional-extra diagnostics, covered both JSON paths with focused regressions, and bumped Serow to `1.0.7-rust-bootstrap` / crate `1.0.7`.
- Verification is recorded in the final run for this change.

- Chose structured enum type replacement as a targeted v1 patch-hardening task because `patch add-type`, `patch remove-type`, and `patch rename-type` already handled enum declarations, while `patch set-type` still rejected enum variants and told agents to edit manually.
- Extended `patch set-type` to replace either record fields or nullary enum variants, while rejecting declaration/name mismatches and record/enum kind changes so renames and deliberate type-kind changes remain explicit structured operations.
- Bumped Serow to `1.0.6-rust-bootstrap` / crate `1.0.6`; verification is recorded in the final run for this change.

- Chose top-level help usage consistency as a public v1 patch hardening task because `serow help --json` returned the command catalog correctly, while `serow help --bogus --json` still exited successfully with text usage instead of the structured `UsageError` protocol used by neighboring discovery commands.
- Added a dedicated help command path that preserves `help --json` and leading `--json help`, rejects extra help arguments with JSON diagnostics when requested, covered the behavior with focused regressions, and bumped Serow to `1.0.5-rust-bootstrap` / crate `1.0.5`.
- Verification is recorded in the final run for this change.

- Chose path-taking CLI option validation as a public v1 patch hardening task because `release-check` already rejected unknown option-looking arguments, while neighboring path-taking commands such as `check`, `fmt`, `plan`, `compile ir`, `query`, and `replay property` still let `--bogus` flow into source discovery as `SourceNotFound`.
- Added shared path argument parsing that reports structured `UsageError` diagnostics for unknown options before `--`, preserved literal dash-prefixed paths after `--`, covered the public command surface with regressions, and bumped Serow to `1.0.3-rust-bootstrap` / crate `1.0.3`.
- Verification is recorded in the final run for this change.

- Chose `release-check` JSON usage consistency as a public v1 patch hardening task because unknown option-looking arguments such as `--bogus` were being treated as source paths, producing `SourceNotFound` diagnostics instead of the structured `UsageError` protocol used by neighboring commands.
- Added release-check-specific option validation that still preserves `--` path separator behavior, covered the JSON usage error and separator path behavior, and bumped Serow to `1.0.2-rust-bootstrap` / crate `1.0.2`.
- Verification is recorded in the final run for this change.

- Chose structured repair-action command allowlist parity as a v1 patch-release hardening task because certification validates repair actions, while the validator still rejected the advertised read-only `docs`, `help`, and `version` discovery commands if a diagnostic used them as command actions.
- Accepted `docs`, `help`, and `version` repair actions in the certification contract validator, added focused regression coverage, and bumped Serow to `1.0.1-rust-bootstrap` / crate `1.0.1`.
- Verification is recorded in the final run for this change.

- Chose public v1 release-baseline closure because all roadmap phases required for the supported bootstrap subset are closed and `bin/serow release-check --json` passes the Serow-owned release gates.
- Promoted `serow.project` to `1.0.0-rust-bootstrap`, bumped the Rust crate manifest to `1.0.0`, updated compact agent/bootstrap wording from backend-closure mode to public v1 release-baseline mode, and recorded targeted v2 hardening as the next implementation posture.
- Verification is recorded in the final run for this change.

- Chose top-level help catalog parity as release polish because `bin/serow help --json` returns the full command catalog, but that catalog did not list the help command itself.
- Added `serow help [--json]` to compact/full command discovery, text usage, README/CLI docs, and project metadata; covered the JSON help catalog with a focused regression.
- Bumped `serow.project` to `0.4.129-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose compact agent bootstrap type-surface parity as release polish because the JSON bootstrap, README, and language docs advertised `List<T>`, while the human `bin/serow agent` output omitted it from supported bootstrap types.
- Updated the text agent bootstrap to include `List<T>`, added a focused regression, and bumped `serow.project` to `0.4.128-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose CLI version reporting as release polish because `serow.project` is the canonical release metadata source but the public CLI had no direct version entrypoint.
- Added `bin/serow version [--json]` plus `bin/serow --version`, exposed it through agent command discovery and docs, and bumped `serow.project` to `0.4.127-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose top-level CLI JSON flag normalization as a release-polish closure because the dispatcher detected `--json` globally for errors, but `serow --json check` and other leading-flag invocations still treated `--json` as the command name.
- Normalized leading/pre-separator top-level `--json` into the selected command's option area while preserving `--` path-separator behavior, covered leading JSON check/query/unknown-command regressions, and bumped `serow.project` to `0.4.126-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose top-level CLI JSON help polish because the public command catalog was available through `agent commands --json`, while `serow help --json` still printed plain usage text and `serow --json` treated the global JSON request as an unknown command.
- Added a JSON help path that returns the full command catalog, changed JSON-only invocation to a structured missing-command `UsageError`, covered both with focused CLI regressions, and bumped `serow.project` to `0.4.125-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose top-level CLI JSON usage consistency as a small release-polish closure because subcommand usage failures already returned structured diagnostics under `--json`, while unknown top-level commands still printed plain text.
- Added a top-level `UsageError` JSON path for unknown commands when `--json` is requested, covered the text/JSON behavior with a focused CLI regression, and bumped `serow.project` to `0.4.124-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose certification CLI profile polish because `certify --profile standard` was accepted and invalid-profile diagnostics named both supported profiles, but usage output, agent command catalogs, README, and CLI docs only advertised `--profile unattended`.
- Added a shared certification usage string with `--profile <standard|unattended>`, updated the JSON usage repair hint plus public docs/project metadata, and covered the catalog output with focused CLI regressions.
- Bumped `serow.project` to `0.4.123-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose v1 release-polish CLI diagnostics because invalid `serow check --profile ... --json` already returned structured JSON but suggested the `certify` command in its repair text.
- Split check/certify usage repair hints so `serow check` usage errors point back to `serow check`, while certification profile errors keep the profile-aware `serow certify` repair.
- Bumped `serow.project` to `0.4.121-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose Phase 2.5 roadmap closure and CLI reference polish because current state already treated the agent-safe language core as v1 done enough, while the roadmap still described several implemented capabilities as open "Started" work and the CLI query examples omitted the public `query effects` command.
- Marked Phase 2.5 done enough for public v1, explicitly deferring semantic embeddings, richer stable node identity, comments-preserving edits, deeper patch coverage, and broader semantic delta classification to v2 hardening unless they block release polish.
- Updated current-state/public docs for `remove_first` and `query effects`, and bumped `serow.project` to `0.4.119-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose Phase 2.6 unattended-safety closure because the strict profile and plan surface now cover the v1 safety gates for evidence weakening, public versioning, capability expansion, implementation evidence, stale migrations, impact coverage, sampled-property replay, low-signal evidence, and repair-action validation.
- Marked Phase 2.6 done enough for public v1, explicitly deferring semantic-embedding reuse search, richer custom generators, proof-based property checking, mutation testing beyond HEAD replay, effect polymorphism, and lower-false-positive intent/implementation analysis to v2+ hardening.
- Updated the compact agent bootstrap's current track to focus future invocations on release polish and targeted v2 hardening, and bumped `serow.project` to `0.4.118-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose Phase 2.6 impact-coverage protocol closure because uncovered dependent-edge diagnostics exposed dependent/target symbols but not the full versioned call path agents need to review transitive impact without reconstructing separate plan rows.
- Added versioned dependent-to-target `path` data to `serow plan --json` `impact_coverage` rows and to `UncoveredImpactEvidence` diagnostics, updated agent diagnostic protocol notes, and marked the impacted-edge coverage roadmap item done enough for v1.
- Bumped `serow.project` to `0.4.117-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose Phase 2 agent-native workflow closure because the remaining roadmap text still treated the agent bootstrap, structured patch coverage, duplicate-intent protection, version/dependent queries, and repair-action diagnostics as open "Started" work even though the v1 tool surface is implemented and discoverable through `bin/serow agent`.
- Marked Phase 2 agent-native workflow done enough for public v1, explicitly deferring semantic embeddings, comment-preserving rewrites, richer AST node identity, and additional repair-action coverage to v2 hardening unless they block unattended safety or release polish.
- Updated the compact agent bootstrap's current track to focus future invocations on Phase 2.6 unattended safety and release polish, and bumped `serow.project` to `0.4.116-rust-bootstrap`.
- Verification is recorded in the final run for this change.

## 2026-05-23

- Chose Phase 1 language-core closure because the compiler gates are warning-free and the implemented Rust bootstrap already covers the roadmap's v1 language-core items, while the roadmap still listed them as open "Started" work.
- Marked Phase 1 closed for public v1, explicitly recording the supported bootstrap textual projection and deferring comments-preserving formatting, source-level generics, payload variants, richer pattern matching/list APIs, effect polymorphism, proofs, custom generators, and JSON-library cleanup to v2+ unless they block another phase.
- Aligned the compact `bin/serow agent` bootstrap and project metadata with v1 closure mode, including the current focus and the v1 `Float`/`List<T>` type surface.
- Bumped `serow.project` to `0.4.115-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose Phase 2.5 certification closure because the roadmap still treated repair-action consistency as an open certification task, while the implementation only validated structured repair actions in the unattended profile.
- Moved structured repair-action command contract validation into every `bin/serow certify` profile while leaving `bin/serow check` as a normal checker-only command.
- Marked the Phase 2.5 agent-certification tightening line done enough for public v1, bumped `serow.project` to `0.4.112-rust-bootstrap`, and updated CLI/progress documentation to describe certification-wide repair-action validation.
- Verification is recorded in the final run for this change.

- Chose v1 closure bookkeeping as the next task because Phase 0 bootstrap tooling and the first Phase 3 Rust backend slice are already implemented and verified, but the roadmap still presented them as open-ended "started" work.
- Marked Phase 0 as closed for public v1 and Phase 3 IR/Rust backend work as done enough for the supported bootstrap subset, with remaining backend targets, recursive records, richer list/generic support, external effects, and semantic-embedding intent search explicitly deferred to v2/future scope.
- Bumped `serow.project` to `0.4.110-rust-bootstrap` and updated README/current-state wording to describe the current codebase as a public v1 bootstrap baseline rather than an unclosed prototype.
- Verification is recorded in the final run for this change.

- Chose a finite `Float` primitive because Serow's AI-first standard library needs decimal quantities, geometry, and trigonometry rather than forcing every numeric API through `Int`.
- Added Float literals, type checking, evaluation, deterministic samples, replay support, portable IR lowering, JSON IR output, Rust backend codegen to `f64`, binary entrypoint support, and Python reference support for the current sample corpus.
- Added compiler-owned pure float math intrinsics for square root, power, trigonometry, inverse trigonometry, and constants, plus source-level `core.float` stdlib wrappers with contracts, executable examples, and sampled properties.
- Updated generated Rust type derives so records containing Float derive `PartialEq` without invalid `Eq`, while Eq is still emitted when every field supports it.
- Updated project metadata, README, language notes, current state notes, and sample-count regression coverage.
- Verification is recorded in the final run for this change.

- Chose a source-level standard library as the next cross-phase task because core helpers existed only as examples or compiler intrinsics, leaving common v1 APIs such as deterministic random, integer helpers, and list wrappers undiscoverable through normal symbol queries.
- Added `examples/stdlib.serow` with `core.bool`, `core.int`, `core.text`, `core.list`, and `core.random` modules, each with explicit versions, contracts, executable examples, sampled properties, effects, and implementations.
- Kept the v1 scope inside current bootstrap semantics: integer-only math, basic text composition, concrete list helpers over existing intrinsics, and seed-threaded deterministic RNG records instead of unsupported floats, slicing, maps, OS entropy, or nondeterministic effects.
- Updated project metadata, README, current state notes, and sample-count regression coverage.
- Verification is recorded in the final run for this change.

- Added `remove_first(list, value)` as a compiler-owned pure list intrinsic, with checker/evaluator/Rust backend support for removing the first matching comparable value from homogeneous lists.
- Migrated `examples/rpg.serow` inventory from a fixed `potion: Bool` field to enum-backed `inventory: List<Item>` state, using `push`, `contains`, and `remove_first` for take/drink behavior while keeping the terminal game flow intact.
- Added temporary bootstrap safe list access through compiler-owned pure intrinsics `get_text(list: List<Text>, index: Int) -> MaybeText` and `get_int(list: List<Int>, index: Int) -> MaybeInt`.
- Chose the explicit `MaybeText`/`MaybeInt` record result path because generic payload enums are not yet in the source language; callers declare `{ found, value }` records until `get(list, index) -> Option<T>` is feasible.
- Updated the checker, evaluator, IR lowering path, Rust backend, examples, and docs so negative, out-of-range, and empty-list access returns `found: false` instead of panicking.
- Verification is recorded in the final run for this change.

## 2026-05-21

- Chose compile CLI JSON usage consistency as a small agent-protocol hardening fix because malformed top-level `serow compile ... --json` commands still emitted plain usage text while related command families return structured diagnostics.
- Updated compile command dispatch to preserve inherited `--json` for `compile ir` and `compile rust`, emit `UsageError` JSON envelopes for missing or unknown compile targets, respect `--` as a path separator, covered the behavior with CLI regressions, and bumped `serow.project` to `0.4.106-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose agent CLI JSON usage consistency as a small agent-protocol hardening fix because malformed `serow agent ... --json` commands still emitted plain usage text while other command families now return structured diagnostics.
- Updated agent command dispatch to emit `UsageError` JSON envelopes for unknown or malformed agent subcommands when `--json` is requested, covered text and JSON regressions, and bumped `serow.project` to `0.4.105-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose replay CLI JSON usage consistency as a small agent-protocol hardening fix because `serow replay property --json` treated `--json` as a sample seed instead of reporting a structured usage diagnostic.
- Updated replay command dispatch to honor inherited JSON requests for missing or unknown replay commands and missing property seeds, covered the behavior with CLI regressions, and bumped `serow.project` to `0.4.104-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose generated Rust source-input bookkeeping as a low-risk backend maintenance cleanup because `compile rust` independently rediscovered and reread the same inputs for per-file metadata and aggregate input fingerprints.
- Consolidated source-input rows and aggregate fingerprint into one internal `SourceInputs` value so generated crate metadata and drift checks share one source walk while preserving existing output.
- Verification is recorded in the final run for this change.

- Chose sampled-property binding validation as a small checker correctness fix because duplicate `forall` variable names were accepted and then silently overwritten during property type checking and execution.
- Added a `DuplicatePropertyVariable` checker error with property index and binding-index data plus a structured removal repair action, covered it with a focused Rust bootstrap regression, and bumped `serow.project` to `0.4.103-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose Python reference module parser parity as a low-risk cleanup because the Rust parser preserves explicit module declarations and `use` dependencies, while the temporary Python bootstrap dropped module-only files and dependency metadata.
- Added Python model support for module dependencies, taught the Python parser to preserve `use` declarations and explicit empty modules, and covered duplicate dependency deduplication plus empty-module preservation in the Python regression suite.
- Verified with focused Python parser tests, `python3 -m unittest discover -s tests`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check`, `bin/serow certify`, and `git diff --check`.

- Chose structured patch metadata validation as a small source-integrity fix because `patch add-function`, `patch set-intent`, and migration patch commands could accept raw newline/control characters that the formatter would write into single-line quoted sections.
- Added shared single-line metadata validation for intents and migration notes, covered the rejecting path through CLI regression tests, and bumped `serow.project` to `0.4.102-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose human-readable plan output parity as a small agent ergonomics fix because JSON property coverage included recursive record sample cycles while text `serow plan` hid them.
- Added recursive record cycle details to text-mode sampled property coverage rows, covered the recursive-record fixture in both JSON and text plan modes, and bumped `serow.project` to `0.4.101-rust-bootstrap`.
- Verification is recorded in the final run for this change.

- Chose generated README robustness as a small Rust backend artifact cleanup because source paths are user-controlled and embedded backticks could break Markdown inline-code spans.
- Added a Markdown inline-code renderer for generated README provenance fields and source inputs, using longer delimiters when values contain backticks.
- Added regression coverage that compiles a `.serow` file with backticks in its filename and verifies the generated README keeps the source path in one code span.
- Verified with `cargo fmt --check`, `cargo test compile_rust_generated_readme_escapes_backtick_source_paths -- --nocapture`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bin/serow check`, `bin/serow certify`, and `bin/serow certify --profile unattended`.

- Chose sampled-property diagnostic specificity as a small production-readiness cleanup because unsupported record property bindings reported only a generic unknown-type reason, leaving agents to rediscover which nested field type was missing.
- Made Rust and Python bootstrap sampleability reasons carry the exact unknown type name, preserving the existing unsupported binding type while clarifying direct and nested unknown-type failures.
- Verification is recorded in the final run for this change.

- Chose source discovery hardening as a small production-readiness cleanup because recursive `.serow` discovery followed directory symlink cycles without tracking visited directories.
- Made Rust source discovery remember canonical directory paths during traversal so ancestor/self symlink loops terminate.
- Added Unix regression coverage that creates a symlink cycle and verifies discovery returns the single real source file.
- Verified with `bin/serow query intent "general cleanup bugfix tech debt production readiness"`, `bin/serow query symbol "parse"`, `cargo fmt --check`, `cargo test source_discovery_ignores_directory_symlink_cycles`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, and `git diff --check`.

- Chose Python reference enum parser parity as a low-risk cleanup because the Rust parser accepts any valid identifier for nullary enum variants, while the temporary Python bootstrap still rejected lowercase variants.
- Updated the Python enum declaration parser to use the shared identifier rule and added a regression that checks lowercase enum variants through parsing, checking, examples, contracts, and sampled properties.
- Verification is recorded in the final run for this change.

- Chose Python reference sampler parity as a low-risk cleanup because Rust supports sampled properties over declared records and enum variants, while the temporary Python bootstrap still treated those declared types as unsupported.
- Added bounded Python record samples, enum variant samples, recursive record cycle reasons, and focused Python regressions for declared-type sampled properties.
- Verification is recorded in the final run for this change.

- Chose patch-command JSON usage consistency as a small agent-protocol hardening fix because `serow patch ... --json` returned structured diagnostics for invalid indexes but still emitted plain usage text for missing or unknown patch commands.
- Updated patch command dispatch and subcommand arity handling so JSON-requested usage failures return the normal patch JSON envelope with `UsageError` diagnostics, while preserving text usage output for non-JSON callers.
- Bumped `serow.project` to `0.4.100-rust-bootstrap`; verification is recorded in the final run for this change.

## 2026-05-20

- Chose text-query usage diagnostics as a small agent-protocol hardening fix because `serow query <text-command> --json` honored JSON for parse errors but still emitted plain stderr usage when the required query text was missing.
- Updated text-query usage failures to return a structured `UsageError` diagnostics JSON envelope whenever `--json` is requested, while preserving plain usage output for text mode.
- Bumped `serow.project` to `0.4.99-rust-bootstrap`; verification is recorded in the final run for this change.

- Chose dependency-free project manifest parser hardening as a small production-readiness cleanup because architecture policy and project version loading depend on `serow.project` before checks run.
- Tightened JSON string parsing for the project manifest so raw control characters are rejected instead of becoming part of version, module, or dependency strings, while preserving valid escaped control sequences.
- Verification is recorded in the final run for this change.

- Chose Python reference parser metadata parity as a low-risk cleanup because the Rust parser already unescapes quoted metadata strings, while the temporary Python bootstrap kept escape sequences in `intent` text and migration notes.
- Added a shared Python quoted-string parser for intent and migration metadata plus focused regression coverage for escaped quotes and backslashes.
- Verification is recorded in the final run for this change.

- Chose `check`/`certify` argument-error consistency as a small agent-protocol hardening fix because invalid `--profile` usage ignored `--json` and emitted plain usage text.
- Updated profile parsing to return explicit usage diagnostics and made `check`/`certify` usage failures emit structured `UsageError` JSON when requested, preserving text usage output otherwise.
- Verification is recorded in the final run for this change.

- Chose Python reference checker parity as a low-risk cleanup because Rust already warns when record fields reference unknown types, while the temporary Python bootstrap only warned for function parameter and return types.
- Added the Python `UnknownType` record-field warning and focused regression coverage so the reference checker keeps the same warning-level behavior for malformed record shapes.
- Verification is recorded in the final run for this change.

- Chose Python reference ledger parity as a low-risk cleanup because Rust `query symbol`/`query symbols` now include declared record/enum types and enum variant hits, while the temporary Python bootstrap still listed only functions.
- Extended the Python reference type model, ledger queries, JSON/text CLI rendering, and regression coverage so declared types and variant-name matches have the same structured row shape as the Rust tool surface.
- Verification is recorded in the final run for this change.

- Chose full symbol-list consistency as a small ledger cleanup because `query symbol` could find declared record/enum types but `query symbols` still listed only public functions.
- Extended the ledger's full symbol listing, text output, and JSON output to include declared type symbols with record/enum shape metadata, and added focused CLI regression coverage.
- Bumped `serow.project` to `0.4.98-rust-bootstrap` and documented the broader `query symbols` behavior.
- Verification is recorded in the final run for this change.

- Chose symbol-query coverage as a Serow tool-interface cleanup because `query symbol` could find public functions but not declared record/enum types or enum variants, even though agents are instructed to use it before adding potentially existing symbols.
- Extended Rust symbol lookup and JSON/text output with type rows, including record fields, enum variants, and type kind metadata, while keeping intent and type-shape queries function-focused.
- Verification is recorded in the final run for this change.

- Chose indexed patch command JSON consistency as a small agent-protocol hardening fix because `patch remove-*` and indexed `patch set-*` argument validation accepted `--json` but still emitted invalid-index usage errors as plain stderr.
- Updated invalid index handling for indexed patch commands to emit a `UsageError` diagnostic in the normal patch JSON envelope when `--json` is present, preserving stderr text mode behavior.
- Verification is recorded in the final run for this change.

- Chose `compile rust --json` argument-error consistency as a small backend CLI hardening fix because invalid backend flags still emitted plain stderr usage text even when callers requested machine-readable JSON.
- Updated `compile rust` argument parsing failures to emit a normal `UsageError` diagnostic JSON envelope when `--json` is present, while preserving existing stderr usage output for text mode, and tightened invalid crate-name/duplicate flag regressions around the JSON contract.
- Verification is recorded in the final run for this change.

- Chose generated Rust crate-name validation as a backend hardening fix because `compile rust --out-dir --crate-name 1bad` passed Serow validation but Cargo rejects package names that start with a digit.
- Tightened `--crate-name` first-character validation to require a lowercase ASCII letter and extended invalid crate-name regression coverage to include digit-leading names.
- Verification is recorded in the final run for this change.

- Chose effect declaration hygiene as a small production-readiness cleanup because redundant declarations such as `effects [io, io]` and `effects [pure, io]` were silently normalized by set-based effect checks.
- Added Rust and Python checker warnings with canonical `patch set-effects` repair actions for duplicate effect capabilities and `pure` mixed with concrete capabilities.
- Verification is recorded in the final run for this change.

- Chose agent diagnostic protocol wording as a low-risk cleanup because `agent diagnostics --json` described duplicate evidence repair actions with a nonexistent `remove-evidence` patch command even though the actual structured actions are `remove-example`, `remove-contract`, and `remove-property`.
- Corrected the machine-readable protocol text and added a regression assertion so command discovery does not drift back to the nonexistent command name.
- Verification is recorded in the final run for this change.

- Chose sampled-property diagnostic maintenance as a low-risk cleanup because checker, replay, and plan each hand-rolled the same unsupported-sample type/reason/cycle aggregation.
- Centralized unsupported-sample summary construction in `src/sampling.rs` and reused it from checker diagnostics, property replay diagnostics, and plan property coverage hints so future sampling support changes only need one diagnostic aggregation path.
- Verified with targeted unsupported-sample and recursive-record tests, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check`, `bin/serow certify`, and `bin/serow certify --profile unattended`.

- Chose parser type-shape hygiene as a small production-readiness cleanup because malformed field, parameter, and return type names could enter the model and surface later as vague unknown-type warnings instead of direct parse errors.
- Added Rust parser validation for simple type identifiers in record fields, function parameters, and return types, mirrored the behavior in the temporary Python bootstrap, and added focused Rust/Python regressions for malformed type names.
- Verification is recorded in the final run for this change.

- Chose repository instruction accuracy as a low-risk production-readiness cleanup because `AGENTS.md` still claimed generated backends did not exist and limited property sampling to `Int`/`Bool`, while the project now has portable IR, Rust backend generation, text/unit/record/enum sampling, and explicit recursive-record sampling diagnostics.
- Updated `AGENTS.md` to describe the current sampled-property domain and backend surface so future agent iterations do not avoid existing compiler functionality or follow stale constraints.
- Verification is recorded in the final run for this change.

- Chose Python reference checker parity as a small production-readiness cleanup because Rust rejects duplicate type declarations, duplicate record fields, and duplicate enum variants, but the temporary Python bootstrap accepted those malformed type declarations.
- Mirrored Rust's `DuplicateType`, `DuplicateRecordField`, and `DuplicateEnumVariant` checks in `serowlang/checker.py`, and added focused Python regressions for each diagnostic.
- Verified with `bin/serow query intent "reject duplicate enum variants" --json`, `bin/serow query symbol DuplicateType --json`, `bin/serow query symbol DuplicateRecordField --json`, `bin/serow query symbol DuplicateEnumVariant --json`, targeted Python duplicate-type tests, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, `cargo test`, and `git diff --check`.

- Chose Python reference parser parity as a small production-readiness cleanup because Rust now rejects duplicate function parameter names, but the temporary Python bootstrap still accepted duplicate parameters and let later maps collapse them inconsistently.
- Mirrored Rust's `DuplicateParameter` parse diagnostic in `serowlang/parser.py`, including the same message shape and repair hint, and added Python regression coverage.
- Verified with `bin/serow query intent "reject duplicate parameter names in Python reference parser"`, `bin/serow query symbol DuplicateParameter`, targeted Rust/Python duplicate-parameter tests, `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, and `git diff --check`.

- Chose formatter repair-action precision as a small production-readiness fix because `fmt --check` diagnostics pointed at the drifted file but suggested `bin/serow fmt` without that path, which can format the default source set instead of the exact failing input.
- Updated `FormatDrift` repair actions to include the drifted source path and tightened regression coverage to assert the exact command argv.
- Verified with `bin/serow query intent "format Serow source files canonically"`, `bin/serow query symbol "fmt"`, `cargo fmt --check`, `cargo test formatter_check_reports_drift_without_writing -- --nocapture`, `cargo clippy --all-targets -- -D warnings`, `bin/serow fmt --check`, `bin/serow check`, `bin/serow certify`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow certify --profile unattended`, and `git diff --check`.

- Chose structured enum type insertion as the next agent-safe cleanup because nullary enum types are now first-class in parsing, checking, IR, and Rust backend output, but `patch add-type` still only accepted record declarations.
- Extended `patch add-type` to accept `Name = Variant | Other` declarations with duplicate-variant rejection before writing, while keeping `patch set-type` scoped to record field replacement.
- Bumped `serow.project` to `0.4.97-rust-bootstrap` and updated README, command discovery, roadmap, language notes, and current state for structured enum insertion.
- Verification is recorded in the final run for this change.

- Chose Rust backend CLI contract cleanup because `compile rust` already accepts `--bin` as an alias for `--emit-bin`, but help output, agent command discovery, and project metadata only advertised the long flag.
- Added a shared `COMPILE_RUST_USAGE` string for the CLI and agent command catalogs so the human and machine-readable surfaces expose `--emit-bin|--bin` consistently.
- Updated project and progress metadata to match the supported alias. Verification is recorded in the final run for this change.

- Chose generated Rust CLI argument hygiene as a production-readiness fix because `compile rust --crate-name` is a single-value backend flag but duplicate occurrences silently let the later value win.
- Updated `compile rust` argument parsing to reject duplicate `--crate-name` flags before validation or artifact generation, matching `--out-dir`, `--check-out-dir`, and `--emit-bin` behavior.
- Bumped `serow.project` to `0.4.96-rust-bootstrap` and updated backend provenance regression expectations.
- Verified with `cargo fmt --check` and `cargo test compile_rust_rejects_duplicate_crate_name_flag -- --nocapture`; full verification is recorded in the final run for this change.

- Chose strict repair-action validation hygiene as a small production-readiness fix because `patch add-module` and `patch set-type` are public structured patch commands but were missing from the unattended certification repair-action allowlist.
- Added both patch commands to `validate_repair_actions` and extended the regression test with synthetic `add-module` and `set-type` command repair actions so future diagnostics can safely point at those commands.
- Verified with `cargo fmt --check`, `cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bin/serow fmt --check`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, `python3 -m unittest discover -s tests`, and `git diff --check`.

- Chose minimal enum/sum type declarations as the next language-core slice because records already flowed through parser, checker, evaluator, IR, sampled evidence, and Rust lowering, while text-game style domains still needed small closed sets such as rooms and commands.
- Added `type Name = Variant | Other` parsing and formatting for nullary enum variants, extended `TypeDecl` with variant metadata, and kept record declarations on the existing `{ field: Type }` path.
- Added enum variant construction by bare variant name, enum equality/inequality, enum values in records, executable examples/contracts/properties, deterministic enum property samples, IR `enum_variant` nodes, and Rust lowering to generated `enum`s deriving `Clone`, `Debug`, `PartialEq`, and `Eq`.
- Added ambiguity diagnostics for duplicate variant names across enum types, variant/function name conflicts, and variant/in-scope variable conflicts. Payload variants and pattern matching remain future work.
- Updated README, language notes, and current progress state, and added regression coverage for enum execution, sampling, IR JSON, generated Rust enum code, and ambiguity diagnostics.
- Verified with `bin/serow query intent "enum sum type variant declaration construction equality Rust backend"`, `bin/serow query symbol "enum"`, `bin/serow query symbol "variant"`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow fmt --check`, `bin/serow check`, and `bin/serow certify`.
- Migrated `examples/rpg.serow` from text/int room and command tags to `Room` and `Command` enums, replacing `command_kind(command: Text) -> Int` with `parse_command(command: Text) -> Command` while keeping terminal text unchanged.
- Updated RPG contracts, examples, properties, generated Rust integration expectations, sample corpus counts, Python reference enum parsing/evaluation, and progress state for enum-backed room state and command parsing.
- Verified with `bin/serow query intent "RPG command room state functions"`, `bin/serow query symbol "RpgState"`, `bin/serow query symbol "command_kind"`, `bin/serow query symbol "room_description"`, `bin/serow check examples/rpg.serow`, `bin/serow compile rust examples/rpg.serow --json`, `bin/serow fmt --check examples/rpg.serow`, `cargo fmt --check`, `bin/serow fmt --check`, `bin/serow check`, `bin/serow certify`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow compile rust examples/rpg.serow --out-dir /tmp/serow_rpg_enum --crate-name serow_rpg_enum --emit-bin`, generated-crate `cargo test --manifest-path /tmp/serow_rpg_enum/Cargo.toml`, and scripted generated-crate run `printf 'north\nfight\n' | cargo run --quiet --manifest-path /tmp/serow_rpg_enum/Cargo.toml`.

## 2026-05-18

- Chose structured record type replacement as the next agent-safe language-core slice because agents could add, remove, and rename record declarations, but changing a record's field shape still required raw text editing even though records now feed checker, IR, Rust structs, and property samples.
- Added `bin/serow patch set-type <path> <module> <type-name> <type-declaration> [--json]`, validating module/type names, rejecting unknown modules, missing type declarations, malformed declarations, duplicate fields, and declaration/name mismatches so renames remain explicit through `patch rename-type`, and rewriting through canonical formatting.
- Bumped `serow.project` to `0.4.95-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, backend provenance regression expectations, and regression coverage for structured record type replacement.
- Verified with `bin/serow query intent "replace a record type declaration through a structured patch command" --json`, `bin/serow query symbol set-type --json`, `cargo fmt --check`, `cargo test patch_set_type_replaces_record_fields -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.

- Chose structured record type renaming as the next agent-safe language-core slice because agents could add and remove record declarations, but renaming a type and its same-file references still required coordinated raw text edits.
- Added `bin/serow patch rename-type <path> <module> <type-name> <new-type-name> [--json]`, validating module/type names, rejecting unknown modules, missing type declarations, and duplicate new type names, rewriting record fields, public signatures, record construction expressions, typed holes, and sampled property headers, and preserving canonical formatting.
- Bumped `serow.project` to `0.4.94-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, repair-action validation, backend provenance regression expectations, and regression coverage for structured record type renaming.
- Verified with `bin/serow query intent "rename a record type through a structured patch command" --json`, `bin/serow query symbol rename-type --json`, `cargo fmt --check`, `cargo test patch_rename_type_rewrites_record_type_references -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.

- Chose structured module dependency replacement as the next agent-safe language-core slice because agents could add or remove dependency declarations, but replacing a stale dependency with the intended module still required two commands or raw text editing.
- Added `bin/serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]`, validating module names, rejecting unknown module targets, missing old dependencies, and duplicate new dependencies, treating same-dependency replacements as no-ops, and rewriting through canonical formatting.
- Bumped `serow.project` to `0.4.93-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, and backend provenance regression expectations for structured module dependency replacement.
- Verified with `bin/serow query intent "replace or update a module dependency through a structured patch command" --json`, `bin/serow query symbol set-use --json`, `cargo test patch_set_use_replaces_existing_dependency -- --nocapture`, and `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`; full verification is recorded in the final run for this change.

- Chose structured module insertion as the next agent-safe language-core slice because agents could safely add functions and types only after a module declaration already existed, but creating the empty module target itself still required raw text editing.
- Added `bin/serow patch add-module <path> <module> [--json]`, validating module names and `.serow` source paths, creating a new source file when needed, appending empty module declarations to existing parsed sources, treating already-present modules as an idempotent no-op, and rewriting through canonical formatting.
- Bumped `serow.project` to `0.4.92-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, and backend provenance regression expectations for structured module insertion.
- Verified with `bin/serow query intent "add a module declaration through a structured patch command" --json`, `bin/serow query symbol add-module --json`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `cargo test patch_add_module_creates_or_appends_empty_module -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `bin/serow agent commands --json`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.

- Chose structured module renaming as the next agent-safe language-core slice because agents could safely rename functions and edit module dependencies, but renaming a module still required raw text edits across declarations, `use` lines, and qualified call references.
- Added `bin/serow patch rename-module <path> <module> <new-module> [--json]`, validating module names, rejecting unknown or duplicate module targets, updating record/function ownership, rewriting in-file `use` declarations, and rewriting in-file exact or module-qualified call references that resolve to functions in the renamed module.
- Bumped `serow.project` to `0.4.91-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, repair-action validation, and regression coverage for structured module renaming.
- Verified with `bin/serow query intent "rename a module through a structured patch command" --json`, `bin/serow query symbol "rename-module" --json`, `cargo fmt --check`, `cargo test patch_rename_module_rewrites_qualified_references -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `bin/serow agent commands --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `cargo test`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.

- Chose structured public function removal as the next agent-safe language-core slice because agents could add, rename, version, and edit public functions through narrow commands, but deleting an experimental public function still required raw text edits despite existing plan/certification gates for removed tracked symbols.
- Added `bin/serow patch remove-function <path> <symbol-or-name> [--json]`, reusing exact/ambiguous function target resolution, removing the function from the parsed module/indexes, and rewriting through canonical formatting while leaving reference fallout and public-symbol-removal policy to `serow check`, `serow plan`, and unattended certification.
- Bumped `serow.project` to `0.4.90-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap/current state metadata, and regression coverage for structured public function removal.
- Verified with `bin/serow query intent "remove a public function through a structured patch command" --json`, `bin/serow query symbol remove-function --json`, `cargo fmt --check`, `cargo test patch_remove_function_removes_public_function -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow agent commands --json`, `cargo test`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.

- Chose structured record type removal as the next agent-safe language-core slice because `patch add-type` gave agents a safe insertion path for record declarations, but stale or experimental record declarations still required raw text deletion.
- Added `bin/serow patch remove-type <path> <module> <type-name> [--json]`, validating module/type names, rejecting unknown modules or missing type declarations with structured patch diagnostics, and rewriting through canonical formatting after removing the declaration from the parsed module and program indexes.
- Bumped `serow.project` to `0.4.89-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap, and current state for structured record type removal.
- Verified with `bin/serow query intent "remove a record type declaration through a structured patch command" --json`, `bin/serow query symbol "remove-type" --json`, `cargo fmt --check`, `cargo test patch_remove_type_removes_record_declaration -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `bin/serow agent commands --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.

- Chose structured record type insertion as the next agent-safe language-core slice because record declarations are now part of the checked language and Rust backend, but agents still had no narrow patch command for adding them without raw text edits.
- Added `bin/serow patch add-type <path> <module> <type-declaration> [--json]`, accepting declarations with or without the `type` prefix, rejecting duplicate type names and duplicate fields, and rewriting through canonical formatting.
- Bumped `serow.project` to `0.4.88-rust-bootstrap` and updated README, agent command discovery, language notes, roadmap, and current state for structured record type insertion.
- Verified with `bin/serow query intent "add a record type declaration through a structured patch command" --json`, `bin/serow query symbol "add-type" --json`, `cargo fmt --check`, `cargo test patch_add_type_inserts_record_declaration -- --nocapture`, `bin/serow agent commands --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.

- Chose explicit recursive-record sample diagnostics as the next cross-phase correctness slice because record sampling and Rust backend layout rejection both knew recursive records were unsupported, but sampled-property diagnostics only exposed a generic unsupported type.
- Added reason-carrying sampleability analysis for property bindings, preserving existing `unsupported_types` output while adding `unsupported_reasons` and `recursive_record_cycles` in checker diagnostics, property replay diagnostics, and `serow plan` property coverage hints.
- Bumped `serow.project` to `0.4.87-rust-bootstrap` and updated README, agent diagnostic text, language notes, roadmap, and current state for explicit non-executable sample reasons.
- Verified with `bin/serow query intent "report recursive record sample cycles in sampled property diagnostics" --json`, `bin/serow query symbol "PropertyNotExecutable" --json`, `bin/serow query symbol "samples_for_type" --json`, `cargo check`, `cargo test recursive_record_property_samples_report_cycle_reason -- --nocapture`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `cargo test`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent diagnostics --json`, and `git diff --check`.

- Chose generated Rust Cargo target discovery hygiene as the next backend package-layout slice because generated crates already detected stale optional `src/main.rs` artifacts, but Cargo could still auto-discover stray source files as unintended targets.
- Updated generated `Cargo.toml` files to disable Cargo automatic target discovery (`autobins`, `autoexamples`, `autotests`, and `autobenches`) and to emit an explicit `[[bin]]` target only when `compile rust --emit-bin` writes `src/main.rs`.
- Bumped `serow.project` to `0.4.86-rust-bootstrap` and updated README, agent bootstrap text, language notes, roadmap, current progress state, and backend regression coverage for explicit generated Cargo targets.
- Verified with `bin/serow query intent "disable automatic Cargo target discovery in generated Rust crates" --json`, `bin/serow query symbol "autobins" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test compile_rust_emit_bin_writes_runnable_crate -- --nocapture`, `cargo test compile_rust_rpg_emit_bin_runs_generated_crate_tests_and_scripted_win -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-cargo-target-discovery-check --crate-name serow_cargo_target_discovery_check --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-cargo-target-discovery-check --crate-name serow_cargo_target_discovery_check --check-out-dir --json`, generated-crate `cargo test`, `bin/serow agent --json`, and `git diff --check`.

- Chose IR-normalized implementation change detection as the next unattended-safety slice because `serow plan` used normalized text for implementation deltas, which could treat non-behavioral expression formatting such as redundant parentheses as a public implementation change.
- Updated plan implementation-change comparison to lower the before and after implementations through the checked expression IR when possible, while preserving existing before/after implementation text in plan JSON and falling back to text comparison if lowering is unavailable.
- Added regression coverage proving a tracked implementation changed from `x + 1` to `(x + 1)` no longer emits `implementation_change` or the public implementation semantic-change label.
- Bumped `serow.project` to `0.4.85-rust-bootstrap` and updated README, language notes, and current progress state for IR-normalized plan comparison.
- Verified with `bin/serow query intent "normalize implementation change detection through IR" --json`, `bin/serow query symbol "implementation_change" --json`, `cargo fmt --check`, `cargo test plan_uses_ir_normalization_for_implementation_changes -- --nocapture`, `cargo test plan_json_reports_implementation_change_against_head -- --nocapture`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, and `bin/serow plan --json`.

- Chose generated Rust crate README provenance as the next backend package-layout slice because generated crates exposed machine-readable Cargo and JSON metadata, while humans inspecting the crate directory still lacked a deterministic source-of-truth and provenance summary.
- Added deterministic `README.md` emission for `compile rust --out-dir` artifacts, including backend/IR/project/input fingerprints, generated counts, source inputs, artifact roles, and binary entrypoint metadata when present; `--check-out-dir` now validates the README alongside `Cargo.toml`, `serow-metadata.json`, and generated Rust sources.
- Bumped `serow.project` to `0.4.84-rust-bootstrap` and updated README, agent bootstrap text, language notes, current state, and backend regression coverage for generated README artifacts.
- Verified with `bin/serow query intent "emit deterministic README provenance for generated Rust crates" --json`, `bin/serow query symbol "README" --json`, `bin/serow query symbol "check-out-dir" --json`, `bin/serow query symbol "RustBackendArtifactDrift" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test compile_rust_emit_bin_writes_runnable_crate -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-readme-check --crate-name serow_readme_check --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-readme-check --crate-name serow_readme_check --check-out-dir --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, generated-crate `cargo test`, and `git diff --check`.

- Chose exact generated Rust evidence-line provenance as the next backend traceability slice because generated evidence-test mappings carried source paths but still pointed at the enclosing function header line instead of the example/property source line.
- Added parser-level evidence line tracking for examples and sampled properties, carried those lines through `serow.ir.v0`, and updated Rust backend JSON/Cargo/sidecar metadata to report generated test rows at the exact evidence line.
- Updated backend/IR regression coverage and docs/project metadata for evidence-line provenance.
- Verified with `bin/serow query intent "record exact source lines for generated Rust evidence test metadata" --json`, `bin/serow query symbol "GeneratedRustTest" --json`, `bin/serow query symbol "IrProperty" --json`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test compile_ir_json_reports_portable_ir -- --nocapture`, `cargo test compile_rust -- --nocapture`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, `bin/serow agent --json`, and `git diff --check`.

- Chose portable IR type provenance as the next backend traceability slice because IR function rows and generated Rust type metadata carried source locations, while `compile ir --json` type rows still omitted the source path and line.
- Added `source_path` and `line` fields to emitted `serow.ir.v0` type declaration rows, with regression coverage for record IR output.
- Updated README, language notes, `serow.project`, and current progress state to document source-location-aware type rows in portable IR.
- Verified with `bin/serow query intent "record source file and line provenance for type declarations in portable IR" --json`, `bin/serow query symbol "type_decl_json" --json`, `bin/serow query symbol "compile ir" --json`, `cargo fmt --check`, `cargo test compile_ir_lowers_record_expressions -- --nocapture`, `bin/serow compile ir examples/text_game.serow --json`, `bin/serow fmt --check --json`, `bin/serow check --json`, `python3 -m unittest discover -s tests`, `git diff --check`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `bin/serow agent --json`.

- Chose Rust backend recursive-record rejection as the next backend correctness slice because record sampling already documented recursive cycles as unsupported, while the Rust backend could otherwise emit invalid by-value recursive structs.
- Added `RustBackendRecursiveRecordType` diagnostics that detect direct or indirect declared-record layout cycles before rendering generated Rust structs, including the detected cycle in diagnostic data.
- Updated backend regression coverage and docs/project metadata for explicit recursive-record layout rejection.
- Verified with `bin/serow query intent "reject recursive record types in generated Rust backend" --json`, `bin/serow query symbol "RustBackendRecursiveRecordType" --json`, `cargo fmt --check`, `cargo test compile_rust_rejects_recursive_record_layouts -- --nocapture`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `bin/serow check --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow certify --json`, `cargo test`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.

- Chose generated Rust project-version provenance as the next backend traceability slice because generated crates already recorded backend id, IR version, and source fingerprints but not the Serow project manifest version that produced those artifacts.
- Added a dependency-free `serow.project` version reader and threaded the version into `compile rust --json`, generated `Cargo.toml` `package.metadata.serow`, and generated `serow-metadata.json`.
- Updated backend regression coverage and docs to assert and advertise deterministic project-version metadata in generated Rust artifacts.
- Verified with `bin/serow query intent "record Serow project version in generated Rust backend artifacts" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "serow.project" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-project-version-check --crate-name serow_project_version_check --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-project-version-check --crate-name serow_project_version_check --check-out-dir --json`, generated-crate `cargo test`, and `git diff --check`.

- Chose Rust generated-crate optional artifact hygiene as the next backend traceability slice because a library-only regeneration after a previous `--emit-bin` run could leave a stale `src/main.rs`, and Cargo would still discover that stale binary target even though the current Serow backend output no longer included it.
- Extended the generated Rust crate artifact model with expected-absent generated files. Library-only `compile rust --out-dir` now removes stale Serow-generated `src/main.rs` files, refuses to remove non-Serow unexpected files, and `--check-out-dir` reports `RustBackendUnexpectedArtifact` when optional generated artifacts are present but not expected.
- Updated backend regression coverage for unexpected `src/main.rs` diagnostics and stale generated binary-entrypoint cleanup when a binary crate is regenerated as a library-only crate.
- Updated README, agent bootstrap text, `serow.project`, and Progress docs to advertise stale optional generated-artifact detection and cleanup.
- Verified with `bin/serow query intent "detect stale generated Rust binary main artifact when checking generated crate layout" --json`, `bin/serow query symbol "check-out-dir" --json`, `bin/serow query symbol "main.rs" --json`, `bin/serow query symbol "RustBackendUnexpectedArtifact" --json`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test compile_rust_emit_bin_writes_runnable_crate -- --nocapture`, `cargo fmt --check`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-unexpected-artifact-smoke --crate-name serow_unexpected_artifact_smoke --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-unexpected-artifact-smoke --crate-name serow_unexpected_artifact_smoke --check-out-dir --json`, generated-crate `cargo test`, and `git diff --check`.

## 2026-05-17

- Chose generated Rust crate artifact drift checking as the next backend traceability slice because emitted crates carried rich provenance but agents still had to overwrite files or manually compare artifacts to know whether a generated crate matched current Serow sources.
- Added `bin/serow compile rust ... --out-dir <dir> --check-out-dir`, which reuses the Rust backend renderer without writing files and compares `Cargo.toml`, `serow-metadata.json`, `src/lib.rs`, and optional `src/main.rs` against the output directory, reporting structured `RustBackendArtifactDrift`, `RustBackendMissingArtifact`, or read diagnostics.
- Extended Rust backend JSON with `checked_files` for check mode, updated agent command usage, README, `serow.project`, and Progress docs, and added regression coverage for clean and stale generated crates.
- Verified with `bin/serow query intent "check generated Rust crate artifacts for drift" --json`, `bin/serow query symbol "check-out-dir" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test agent_ -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-check-out-dir-smoke --crate-name serow_check_out_dir_smoke --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-check-out-dir-smoke --crate-name serow_check_out_dir_smoke --check-out-dir --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.
- Chose generated Rust crate metadata sidecars as the next backend artifact-layout slice because generated crates persisted provenance in Cargo metadata but external tools still had to parse TOML or rerun `compile rust --json` to inspect it.
- Added deterministic `serow-metadata.json` emission for `compile rust --out-dir` artifacts, mirroring backend id, IR version, crate name, aggregate/per-source input fingerprints, generated source fingerprint, generated counts, type/function mappings, binary entrypoint provenance, and generated evidence-test mappings.
- Updated Rust backend integration coverage to assert the sidecar is written for library and binary crate emission and is reported in `written_files`.
- Updated README, agent bootstrap text, `serow.project`, and Progress docs to advertise JSON sidecar metadata for generated Rust crates.
- Verified with `bin/serow query intent "emit deterministic Rust backend module manifest metadata for generated crates" --json`, `bin/serow query intent "record generated Rust backend metadata in JSON and Cargo manifest" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "RustBackend" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test compile_rust_emit_bin_writes_runnable_crate -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-metadata-sidecar-check --crate-name serow_metadata_sidecar_check --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, generated-crate `cargo test`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `git diff --check`, and `python3 -m json.tool /tmp/serow-metadata-sidecar-check/serow-metadata.json`.
- Chose generated Rust source-input metadata as the next backend traceability slice because generated artifacts carried an aggregate Serow input fingerprint but did not expose the deterministic source input list that produced it.
- Extended `compile rust --json` output with `rust.inputs` rows containing each discovered Serow source path, byte count, and per-file FNV-1a fingerprint while preserving the existing aggregate input fingerprint algorithm.
- Extended generated Rust crate manifests with deterministic `[[package.metadata.serow.inputs]]` rows containing the same path/fingerprint/byte-count metadata.
- Updated README, agent bootstrap text, `serow.project`, and Progress docs to advertise aggregate and per-source Serow input metadata for generated Rust artifacts.
- Verified with `bin/serow query intent "record Serow source input paths and per-file fingerprints in generated Rust backend artifacts" --json`, `bin/serow query symbol "source input fingerprint" --json`, `bin/serow query symbol "compile rust" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `bin/serow compile rust examples/math.serow --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-input-metadata-check --crate-name serow_input_metadata_check --json`, generated-crate `cargo test`, `bin/serow plan --json`, and `git diff --check`.
- Chose generated Rust input traceability as the next backend metadata slice because generated crates recorded the generated `src/lib.rs` fingerprint but not the exact Serow source inputs used to produce the artifact.
- Added a deterministic FNV-1a input fingerprint over discovered `.serow` source paths and bytes, exposed it in `compile rust --json`, and recorded it as `package.metadata.serow.input_fingerprint` in generated Cargo manifests.
- Updated README, agent bootstrap known limits, `serow.project`, and Progress docs to advertise Serow input fingerprint metadata.
- Verified with `bin/serow query intent "record Serow source input fingerprint in generated Rust backend artifacts" --json`, `bin/serow query symbol "input_fingerprint" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo test compile_rust -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir /tmp/serow-fingerprint-check --crate-name serow_fingerprint_check --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow agent --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.
- Chose ownership-aware final record-update lowering as the next backend slice because `World -> World` style functions still cloned whole records in final return position even when generated postcondition checks no longer needed the original base value.
- Updated the Rust backend to detect top-level function bodies shaped as `record with { ... }`, evaluate update values before moving the base, and emit Rust struct update syntax with `..base` instead of `..base.clone()` when no lowered postcondition references the original base variable.
- Added regression coverage proving the backend keeps clone-based lowering when postconditions still inspect the original input, while moving the base record for fixed-output final updates.
- Updated README, `serow.project`, agent bootstrap text, and Progress docs to advertise ownership-aware final record update lowering.
- Verified with `bin/serow query intent "avoid cloning records in generated Rust record update expressions" --json`, `bin/serow query symbol "record update" --json`, `bin/serow query symbol "RustBackend" --json`, `cargo fmt --check`, `cargo test compile_rust_emits_record_structs_and_operations -- --nocapture`, `cargo test compile_rust -- --nocapture`, `bin/serow check --json`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow compile rust examples/text_game.serow --json`, and `git diff --check`.
- Chose resolved Rust binary entrypoint provenance as the next backend slice because generated binary crates recorded the Serow entrypoint symbol and return type but not the resolved Rust function name or source location as first-class binary metadata.
- Extended `compile rust --emit-bin --json` with a `binary_entrypoint` object containing symbol, Rust name, return type, source path, and line.
- Extended generated `Cargo.toml` `package.metadata.serow` binary entrypoint metadata with the same Rust-name and source-location fields.
- Updated README, `serow.project`, and Progress docs to advertise binary entrypoint provenance metadata.
- Chose declared-record property sampling as the next cross-phase language/backend slice because record state is now central to Serow examples but sampled `forall` evidence could not quantify over declared record types.
- Made deterministic property sampling program-aware: declared record types now receive bounded samples consisting of a default record plus one-field-at-a-time variants from deterministic field samples, while recursive record sample cycles remain unsupported.
- Updated checker execution, single-sample property replay, plan property coverage, shrinking, HEAD-sensitivity analysis, and generated Rust property tests to use the same declared-record sample sets.
- Added regression coverage proving record-typed properties are checkable, replayable, reported with correct plan sample counts, and emitted as compilable Rust tests.
- Updated README, agent bootstrap text, `serow.project`, and Progress docs to document bounded declared-record property samples.
- Chose the first ownership-friendly Rust record-state lowering slice because records and loops are now central to terminal programs, but generated field reads and same-variable record updates still cloned more state than necessary.
- Updated the Rust backend so direct field reads from local record variables access the field without cloning the whole record, and `set state = state with { ... }` lowers to in-place field assignments after evaluating update values.
- Added regression coverage for generated record Rust and the RPG terminal loop to verify direct field reads, in-place state updates, old-value swap semantics, generated Rust compilation, and generated evidence execution.
- Chose generated Rust record-type manifest provenance as the next cross-phase backend task because JSON output already exposed generated type mappings but generated crates did not persist those rows in `Cargo.toml`.
- Extended generated `package.metadata.serow` with a deterministic `generated_types` count and `[[package.metadata.serow.types]]` rows containing Serow type symbols, generated Rust type names, source paths, and line numbers.
- Updated README, `serow.project`, and Progress docs to advertise source-location-aware generated type metadata in Rust crate manifests.
- Extended Rust binary backend entrypoints to accept declared record return types in addition to `Text`, `Int`, `Bool`, and `Unit`.
- Generated `src/main.rs` now prints scalar entrypoint results with `Display`, record results with derived `Debug`, and leaves `Unit` entrypoints to explicit effects.
- Added integration coverage that compiles a record-returning Serow `main`, writes a generated Rust crate, verifies manifest metadata and `main.rs`, and runs the binary.
- Refreshed the Python reference bootstrap for the current sample corpus by parsing record declarations, evaluating record construction/access/update, and recognizing current sample evidence counts.
- Updated CLI help, README, and Progress docs to advertise record-returning Rust binary entrypoints.
- Added `examples/rpg.serow`, a deterministic terminal RPG demo with two rooms, HP/gold/inventory state, command parsing, win/loss/end states, and a `pub fn main() -> Unit` entrypoint for generated Rust binaries.
- Added seed-threaded pure randomness helpers `next_random(seed: Int) -> Int` and `random_range(seed: Int, max: Int) -> Int` instead of adding ambient randomness.
- Added pure RPG helper evidence for command parsing, room/status descriptions, deterministic combat, and state transitions, plus Rust backend integration coverage for generated helper source, generated Rust tests, and a scripted winning binary run.
- Updated README with the command sequence for building and running the RPG demo through `bin/serow compile rust --emit-bin`.
- Added minimal structured record state support for small RPG-style models.
- The Rust parser/model/formatter now handle top-level `type Name = { field: Type }` record declarations.
- Expressions now support explicit record construction, field access, and copy-update with static checking, evaluation, IR lowering, and Rust backend emission.
- The Rust backend emits generated structs for declared record types and lowers copy-update through clone-based Rust struct update syntax.
- Added `Player` and `GameState` examples in `examples/text_game.serow`, including `GameState` updates inside checked loop code.
- Added regression coverage for record construction/access/update, record type errors, IR JSON nodes, generated Rust structs, and record state in loops.
- Added a checked loop mechanism for terminal text games: `while <Bool> do (<Unit>)` returns `Unit`, and `set name = expr` updates an existing local `let` binding with same-type values only.
- Loops now type-check, evaluate with a finite evidence iteration limit, lower to `serow.ir.v0`, render to Rust `while` loops, and preserve direct-call effect discovery for nested terminal I/O.
- Added `examples/text_game.serow` as a tiny interactive skeleton that prints a room, reads a command, updates loop state, and exits on `quit` or the checker model's empty input.
- Added regression coverage for loop execution, loop type diagnostics, local-assignment diagnostics, effect handling, IR JSON, and Rust backend lowering.
- Updated the Python reference bootstrap enough to keep the current sample corpus executable for `Unit`, terminal intrinsics, multi-line sequencing, and the checked loop skeleton.
- Added minimal sequencing for terminal-style programs with `let name = expr; next` and `unit_expr; next`.
- Sequenced expressions now type-check, evaluate, lower to `serow.ir.v0`, render to Rust blocks, and participate in direct-call effect discovery so nested `print`/`read_line` calls require `effects [io]`.
- Added `examples/terminal_io.serow::greet_user` as a checked sample of `print; let read_line; print`.
- Added regression coverage for binding scope, non-`Unit` discard type errors, effect propagation through sequencing, canonical formatting, IR JSON, and Rust backend rendering.
- Added the first checked terminal I/O intrinsics: `print(text: Text) -> Unit` and `read_line() -> Text`.
- Added minimal `Unit` support across expression tokens, type checking, evaluator values, deterministic sampling, IR JSON, and Rust backend lowering.
- Intrinsics are compiler-owned ledger-queryable symbols and do not require `use serow.intrinsic`; direct callers must still declare `effects [io]`.
- The checker uses a non-interactive intrinsic model (`print` returns `unit`, `read_line` returns empty `Text`) so examples and sampled properties do not block or write terminal output during verification.
- The Rust backend now permits the narrow terminal `io` slice, lowers `print` to `println!`, lowers `read_line` to stdin reading with newline trimming, and skips generated Rust evidence tests for `io` functions.
- Binary Rust emission now accepts `pub fn main() -> Unit`, calling the generated entrypoint without adding a second result print.
- Added `examples/terminal_io.serow` plus Rust integration tests for intrinsic effect under-declaration, intrinsic ledger queries, Rust codegen for `print`/`read_line`, and runnable `Unit` terminal entrypoints.

## 2026-05-03

- Started from an empty repository containing only `Progress/originalConversation.txt`.
- Chose dependency-free Python for the bootstrap because Python 3.9 is available and Node/Rust are not.
- Added repository agent instructions, `serow.project`, roadmap, a sample Serow program, and a minimal compiler CLI.
- Initial compiler goals: parse public functions, enforce required sections, execute examples, sample properties, and expose semantic queries.
- Implemented the bootstrap parser/checker/ledger in `serowlang/`.
- Added `examples/math.serow` with `add` and `abs`.
- Added unit tests covering successful checking, failed examples, and intent queries.
- Verified with unit tests, `bin/serow check`, `bin/serow certify`, query commands, and redirected-cache `compileall`.
- Installed Rust toolchain became available, so the bootstrap was migrated to a dependency-free Rust Cargo project.
- Added Rust modules for model, diagnostics, parser, evaluator, checker, ledger, and CLI.
- Updated `bin/serow` to invoke the Rust CLI through Cargo.
- Added Rust integration tests mirroring the Python bootstrap tests.
- Verified Rust implementation with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `bin/serow check`.
- Added an internal static type checker for the bootstrap expression subset.
- The checker now validates implementation return types, boolean contracts/examples/properties, function call arity, and function call argument types before executable evidence runs.
- Added Rust integration tests for implementation return-type mismatches and bad function-call argument types.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow check`, `bin/serow check --json`, and `bin/serow certify`.
- Added parsed, type-checked, and executable `requires` preconditions to the Rust bootstrap.
- Function calls now fail before implementation evaluation when their declared preconditions are false.
- Added `div_trunc` to `examples/math.serow` as the first sample function with a non-zero divisor precondition.
- Updated the Python reference bootstrap to parse and enforce `requires` and to mirror Rust's truncating integer division/remainder behavior.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow check --json`, `bin/serow query symbol div_trunc --json`, `bin/serow query intent "truncating integer division nonzero divisor" --json`, and `bin/serow certify`.
- Added dependency-free deterministic formatting for the Rust bootstrap textual projection.
- Added `bin/serow fmt [paths...]` to rewrite `.serow` files and `bin/serow fmt [paths...] --check` to report canonical-format drift without writing.
- Added Rust integration coverage for formatter check mode and formatter rewriting.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, and `bin/serow certify`.
- Added explicit top-level `use <module>` declarations to the Rust parser and AST.
- Added dependency-free parsing for `serow.project` module architecture policies.
- `bin/serow check` now reports `ArchitectureViolation` when a module with a configured policy imports a module outside its `may_depend_on` list.
- `bin/serow fmt` preserves and canonicalizes module `use` declarations.
- Added Rust integration tests for architecture policy enforcement, project architecture parsing, and formatting `use` declarations.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow query intent "add two integers" --json`, `bin/serow query symbol abs --json`, `bin/serow query symbols --json`, and `bin/serow certify`.
- Added Apache-2.0 licensing via root `LICENSE`, Cargo package metadata, and README license notes.

## 2026-05-04

- Added expression call discovery to the Rust bootstrap evaluator support.
- The checker now infers cross-module dependencies from function calls found in implementations, `requires`, `ensures`, examples, and property bodies.
- Added `MissingModuleDependency` diagnostics when a cross-module call is allowed but the caller module has no matching `use <module>` declaration.
- Extended `ArchitectureViolation` checking so omitted `use` declarations cannot hide calls to modules forbidden by `serow.project`.
- Added Rust integration tests for missing inferred dependencies, declared cross-module calls, and inferred architecture violations.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow query intent "add two integers" --json`, `bin/serow query symbol abs --json`, `bin/serow query symbols --json`, and `bin/serow certify`.
- Added conservative effect capability checking to the Rust checker and Python reference checker.
- Functions declared `effects pure` now produce `EffectViolation` when direct calls in implementations, contracts, examples, or properties resolve to a function whose effects are not exactly `pure`.
- Added Rust and Python regression tests for pure functions calling an `[io]` function.
- Updated `Progress/language-v0.md` to reflect implemented cross-module dependency inference and the new bootstrap effect rule.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow query intent "add two integers" --json`, `bin/serow query symbol abs --json`, `bin/serow query symbols --json`, and `bin/serow certify`.
- Added `bin/serow agent [--json]` as the first Phase 2 agent-native workflow command.
- The agent command prints the current language/toolchain contract, workflow requirements, CLI command list, public function requirements, verification gates, and known bootstrap limits.
- Added Rust integration tests for the text and JSON forms of `bin/serow agent`.
- Updated README and Progress docs to point future sessions at the agent bootstrap command.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow agent`, `bin/serow agent --json`, and `bin/serow certify`.

## 2026-05-05

- Added the first structured patch command: `bin/serow patch add-use <path> <module> <dependency> [--json]`.
- The patch command parses the target Serow file, rejects parse errors and unknown module targets, updates the module dependency list, and rewrites the file through the canonical formatter.
- `MissingModuleDependency` diagnostics now point agents at the `patch add-use` command.
- Added Rust integration coverage for `patch add-use` repairing a missing cross-module dependency.
- Updated `bin/serow agent [--json]`, README, `serow.project`, and Progress docs to advertise the patch interface.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow agent --json`, query commands, and `bin/serow certify`.
- Added exact normalized duplicate public intent detection to the Rust checker and Python reference checker.
- Duplicate public intents now produce `PossibleDuplicate` errors with repair guidance pointing agents to `bin/serow query intent "<description>"`.
- Added Rust and Python regression tests for duplicate public intents.
- Updated `bin/serow agent --json`, README, `serow.project`, and Progress docs to document duplicate-intent enforcement and its current exact-match limit.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow query intent "add two integers" --json`, `bin/serow query symbol abs --json`, `bin/serow query symbols --json`, `bin/serow agent --json`, and `bin/serow certify`.
- Added `bin/serow query dependents <symbol-or-name> [paths...] [--json]` to the Rust ledger.
- The dependents query reports direct call sites found in implementations, `requires`, `ensures`, examples, and sampled property bodies, using the same unambiguous-name call resolution rule as checker dependency inference.
- Added explicit fixed `v1` version metadata to function JSON in query results, symbol listings, and dependent query function references.
- Added Rust integration coverage for dependent call-site reporting across implementation and property contexts.
- Updated `bin/serow agent [--json]`, README, `serow.project`, and Progress docs to advertise the dependent query and fixed-version metadata.
- Added machine-readable diagnostic `repair_actions` alongside legacy `repairs` strings in Rust JSON output.
- Command repair actions now cover missing module dependency fixes via `patch add-use`, duplicate-intent ledger lookups via `query intent`, and format drift fixes via `fmt`.
- Added Rust integration coverage proving `bin/serow check --json` emits structured command repair actions.
- Updated `bin/serow agent [--json]`, README, `serow.project`, and Progress docs to document the diagnostic repair action contract.
- Reframed the roadmap to add Phase 2.5: Agent-Safe Language Core before production backend work.
- Phase 2.5 prioritizes explicit symbol identity, qualified calls, stronger ledger queries, repair-action diagnostics, shared AST/IR boundaries, structured patch expansion, and tighter certification.
- Backend work remains planned, but Rust transpilation is now explicitly downstream of stable identity and evidence semantics.
- Added source-level symbol versions to the Rust bootstrap with an optional `version vN` function section.
- `Function::symbol()` now uses the parsed source version, while omitted versions continue to default to `v1` for compatibility.
- The canonical formatter preserves explicit `version vN` sections, and the sample Serow program now declares `version v1` on public functions.
- Updated the Python reference bootstrap to parse declared versions and include them in symbol identity.
- Added Rust and Python regression tests proving a `version v2` declaration produces `@module.name.v2` ledger identity.
- Added `AmbiguousUnqualifiedName` checking in Rust and Python so duplicate bare function names are rejected until qualified references exist.
- Updated agent bootstrap output and Progress docs to describe Phase 2.5 source-level identity semantics.
- Added qualified function references to the Rust bootstrap expression subset.
- Calls now resolve through one shared rule across evaluation, static type checking, effect checking, inferred module dependencies, and dependent queries.
- Supported call forms are bare `name(...)` when unambiguous, module-qualified `module.name(...)` / `module.name.vN(...)`, and exact canonical `@module.name.vN(...)`.
- Replaced duplicate bare-name rejection with `AmbiguousUnqualifiedCall` diagnostics for ambiguous bare call sites.
- Updated the Python reference bootstrap to evaluate qualified calls and mirror the ambiguous-call diagnostic.
- Added Rust and Python regression tests showing duplicate function names work when exact symbol calls are used, while ambiguous bare calls are still rejected.
- Updated `bin/serow agent --json`, README, `serow.project`, and Progress docs to document qualified references.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, query commands, `bin/serow agent`, `bin/serow agent --json`, and `bin/serow certify`.
- Added Phase 2.6: Unattended Agent Safety to the roadmap.
- The new roadmap phase makes low-attention AI implementation safety explicit, with planned work for evidence-weakening detection, change-impact gates, public versioning policy, semantic reuse checks, structured capabilities, machine-readable change plans, evidence-drift guards, and a strict unattended certification profile.
- Improved intent ledger search in the Rust bootstrap and Python reference implementation.
- `bin/serow query intent` now uses deterministic weighted token ranking with stopword filtering, light plural/gerund normalization, and type-token aliases such as `integer`/`integers` to `int`.
- Intent query results now use stable symbol tie-breaking and no longer return matches only because of low-value words such as `by`.
- Added Rust and Python regression tests for ranked content-token matching and stopword-only query rejection.
- Added `bin/serow query impact <symbol-or-name> [paths...] [--json]` to report direct and transitive dependents through resolved reverse call paths.
- Impact rows include dependent, target, depth, a function-reference path from dependent to target, and immediate call sites for the first edge on that path.
- Updated agent bootstrap output, README, `serow.project`, and Progress docs to advertise the impact query as the first change-impact ledger primitive.
- Added `bin/serow patch add-function <path> <module> <signature> <intent> [--json]` as the second structured patch command.
- The add-function patch validates the target module and signature, rejects duplicate `v1` public symbols, inserts explicit `version v1`, preserves the supplied intent, declares `effects pure`, and leaves `impl` as a typed hole without inventing contracts, examples, or properties.
- Added Rust integration coverage proving the generated skeleton is canonical and still fails `check` with `MissingRequiredSection` and `TypedHole` until real evidence and implementation are supplied.
- Added structured evidence and hole-filling patch commands:
  - `bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]`
  - `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]`
  - `bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]`
- Evidence patches append only when the exact evidence is not already present, reject ambiguous bare function targets, and rewrite through the canonical formatter.
- `patch fill-hole` replaces typed implementation holes but rejects non-hole implementations instead of overwriting existing behavior.
- Added Rust integration coverage for completing a generated public skeleton through structured patch commands and for ambiguous patch-target diagnostics.
- Added the first strict certification profile: `bin/serow certify --profile unattended`.
- The unattended profile reuses normal checker/certification behavior and adds a `MissingExplicitVersion` error for any public function that still relies on the bootstrap default `v1` version.
- Updated agent bootstrap output, README, and Progress docs to advertise the unattended profile as the first Phase 2.6 strict certification gate.
- Added Rust integration coverage proving normal certification still accepts implicit versions for compatibility while unattended certification rejects them, and proving the sample program passes the unattended profile.
- Added `bin/serow patch set-version <path> <symbol-or-name> <version> [--json]`.
- The set-version patch makes an existing function's source-level version explicit, rejects invalid versions, duplicate canonical symbols, and dependent-unaware version changes.
- `MissingExplicitVersion` diagnostics in unattended certification now include a concrete command repair action pointing at `patch set-version`.
- Added Rust integration coverage for applying the set-version repair and for the unattended repair action JSON.
- Added `bin/serow plan [paths...] [--json]` as the first machine-readable change plan primitive for Phase 2.6.
- Explicit path arguments are treated as the selected change set; without paths, the command uses Git status to find changed `.serow` files.
- The plan report includes checker diagnostics, changed public symbols, evidence counts, explicit-version state, transitive impact rows, and residual-risk strings.
- Updated `bin/serow agent [--json]`, README, `serow.project`, and Progress docs to advertise the plan command and mark the active phase as Phase 2.6.
- Added Rust integration coverage proving `serow plan --json` reports changed symbols, evidence coverage, and dependent impact.
- Extended `bin/serow plan [paths...] [--json]` with the first baseline-aware evidence weakening report.
- Plan now parses `HEAD` versions of tracked changed `.serow` files, compares matching public symbols by canonical identity, emits baseline evidence counts, evidence deltas, and removed/narrowed evidence rows, and adds residual risks when executable evidence is weakened.
- Exposed an in-memory parser entry point so the plan command can parse Git baseline content without temporary files.
- Added Rust integration coverage using a throwaway Git repository to prove removed examples and contracts are reported against `HEAD`.
- Made evidence weakening a strict unattended certification gate.
- `bin/serow certify --profile unattended` now reuses the baseline evidence analysis for changed tracked public symbols and emits `EvidenceWeakening` errors when examples, contracts, properties, or preconditions are removed or narrowed compared with Git `HEAD`.
- Normal certification remains unchanged for local iteration; explicit-version enforcement still runs as the other unattended gate.
- Added Rust integration coverage proving standard certification passes a behavior-preserving evidence removal while unattended certification rejects it with `EvidenceWeakening` diagnostics.
- Made unchecked transitive impact a strict unattended certification gate.
- Git-status planning now analyzes tracked project `.serow` files in addition to changed `.serow` files, so unchanged dependents can be discovered instead of hidden by a narrow changed-file parse.
- `bin/serow certify --profile unattended` now emits `UncheckedImpact` errors when a changed tracked public symbol has transitive dependents outside the certified change set, with diagnostic data for the symbol, dependent, depth, path, and call sites.
- Added Rust integration coverage proving unattended certification rejects a changed public function with an unchanged dependent in another tracked Serow file.
- Added impact-edge evidence coverage rows to `bin/serow plan [paths...] [--json]`.
- The plan now reports whether impacted dependent call edges are covered by direct example/property calls, or by examples/properties that exercise an implementation, precondition, or contract edge through the dependent function.
- Uncovered impact edges now add per-symbol residual risk text so shallow dependent evidence is visible before unattended certification.
- Added Rust integration coverage for covered and uncovered impact-edge coverage rows.
- Added same-version public contract-surface change detection for tracked public symbols.
- `bin/serow plan [--json]` now emits `behavior_change` rows when a changed public symbol keeps the same canonical version but changes its signature, requires clauses, ensures clauses, examples, properties, or effects compared with Git `HEAD`.
- `bin/serow certify --profile unattended` now rejects those changes as `PublicBehaviorChangeNeedsVersion`; bumping the public `version vN` creates a new canonical symbol and satisfies this gate.
- Updated agent bootstrap output, README, `serow.project`, and Progress docs to document the stricter public versioning policy.
- Made uncovered impacted call edges a strict unattended certification gate.
- `bin/serow certify --profile unattended` now emits `UncoveredImpactEvidence` when a changed public symbol has an impacted dependent call edge that is inside the certified change set but is not exercised by an executable example or sampled property.
- Added Rust integration coverage proving a change set that includes both target and dependent still fails unattended certification when the dependent evidence is shallow.
- Updated agent bootstrap output, README, and Progress docs to document the new impact coverage gate.
- Added normalized public implementation-change detection against Git `HEAD` to `bin/serow plan [--json]`.
- Changed-symbol plan rows now include `implementation_change` with before/after implementation text when a tracked public function keeps the same canonical symbol but changes its body.
- `bin/serow certify --profile unattended` now emits `ImplementationChangeNeedsEvidence` when a changed tracked public symbol modifies its implementation without adding executable evidence.
- Added Rust integration coverage proving plan JSON reports implementation drift and unattended certification rejects an implementation-only public change while normal certification still passes.
- Added source-level function `migration` records for `public-behavior-change`, `evidence-weakening`, `implementation-change`, and `impact-review` acknowledgements.
- The Rust and Python parsers now read migration records, the Rust formatter preserves them, and `serow plan --json` exposes them on changed public symbols.
- Added `bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]`.
- Unattended certification now treats matching migration records as explicit acknowledgements for intentional public behavior, evidence weakening, implementation, and impact-review gate decisions.
- Added command-style repair actions for unattended gate diagnostics that point to `patch add-migration`.
- Updated agent bootstrap output, README, `serow.project`, and Progress docs to document migration acknowledgements.
- Added declared capability-change detection against Git `HEAD` to `bin/serow plan [--json]`.
- `serow plan` now emits `capability_change` rows with before/after effects and added/removed capabilities for tracked changed public symbols.
- `bin/serow certify --profile unattended` now emits `CapabilityExpansionNeedsMigration` when a changed tracked public symbol adds declared capabilities without a `capability-expansion` migration acknowledgement.
- The `capability-expansion` migration kind is accepted by the Rust bootstrap, the structured patch command, and the Python reference parser.
- A `capability-expansion` migration also acknowledges an effects-only public surface change for the same canonical symbol, while broader public surface changes still require the existing public behavior migration or a new version.
- Added implementation/evidence drift detection for changed tracked public symbols.
- `serow plan --json` now emits `evidence_drift` rows when a same-symbol implementation change is paired with changed executable evidence.
- `bin/serow certify --profile unattended` now emits `ImplementationEvidenceDriftNeedsMigration` unless the function has an `implementation-change` migration acknowledgement.
- Added Rust integration coverage proving added evidence no longer masks a same-version implementation change in unattended certification.
- Added implementation evidence coverage analysis for changed tracked public symbols.
- `serow plan --json` now emits `implementation_evidence` rows for implementation changes, including added examples, added properties, direct call coverage, and a reason string.
- `bin/serow certify --profile unattended` now emits `ImplementationChangeNeedsCoveringEvidence` when a same-symbol implementation change adds executable examples/properties that do not directly call the changed function.
- Added Rust integration coverage proving a tautological added example no longer satisfies the implementation-change evidence gate.
- Added structured diagnostic repair-action contract validation for unattended certification.
- `bin/serow certify --profile unattended` now emits `RepairActionContractViolation` if a diagnostic repair action has an unsupported kind, empty label/argv component, non-`bin/serow` command prefix, missing subcommand, or unknown `patch`/`query` subcommand.
- Added Rust coverage for both valid command repair actions and malformed synthetic repair actions, and confirmed the real missing-version repair action passes the stricter contract.
- Added near-duplicate public intent warnings to the Rust checker and Python reference checker.
- Exact normalized duplicate public intents remain `PossibleDuplicate` errors, while high-overlap token-ranked matches now emit `NearDuplicateIntent` warnings with candidate symbol, score, overlap reasons, and a `query intent` structured repair action in Rust.
- Added Rust and Python regression tests proving a similar-but-not-exact `sum_pair` intent points back to an existing `add` symbol without turning into an exact duplicate error.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to advertise near-duplicate semantic reuse warnings.
- Added HEAD-sensitivity analysis for added implementation evidence.
- `serow plan --json` now reports whether added examples/properties for a changed implementation would fail against the Git `HEAD` implementation for the same canonical symbol.
- `bin/serow certify --profile unattended` now emits `ImplementationChangeNeedsSensitiveEvidence` when added implementation evidence directly calls the changed function but also passes against the `HEAD` implementation, unless acknowledged by an `implementation-change` migration.
- Added Rust integration coverage for both HEAD-insensitive implementation evidence and evidence that distinguishes the changed implementation from `HEAD`.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document the new lightweight shallow-evidence check.
- Tightened effect capability checking from a pure/effectful split to direct-call capability subset validation.
- The Rust checker and Python reference checker now emit `EffectViolation` when any function calls a resolved callee without declaring all of the callee's concrete non-`pure` capabilities.
- Added Rust and Python regression coverage showing an `[io]` caller cannot call a `[network]` callee unless it declares `network`, while a caller declaring `[io, network]` is accepted.
- Updated the agent bootstrap contract, README, and Progress docs to document structured direct-call capability validation.
- Added conservative unused declared-capability warnings to the Rust checker and Python reference checker.
- `UnusedEffectCapability` now warns when a function has resolved non-self direct callees and declares concrete capabilities that none of those callees require, while still allowing effectful leaf functions until Serow has external effect primitive syntax.
- Added Rust and Python regression coverage for an over-declared `[io, network, disk]` wrapper that only calls `[io]` and `[network]` callees.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document direct-call capability minimality warnings.
- Added `bin/serow patch set-effects <path> <symbol-or-name> <effects> [--json]`.
- The set-effects patch replaces a function's explicit effect declaration through the structured patch interface, accepting `pure` or a bracketed concrete capability list such as `[io, network]`.
- `EffectViolation` and `UnusedEffectCapability` diagnostics now include command-style repair actions pointing at `patch set-effects`, and unattended repair-action contract validation accepts the new patch subcommand.
- Added Rust integration coverage proving `patch set-effects` repairs an effect capability diagnostic.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document structured effect declaration patches.
- Made `bin/serow patch set-version` dependent-aware for public version bumps.
- The command still makes implicit versions explicit, but it can now move a function to a new `vN` when parsed call sites do not pin the old canonical symbol.
- Version bumps are rejected as `VersionPinnedDependent` when a parsed call site uses `module.name.vN(...)` or exact `@module.name.vN(...)` for the current symbol, with data listing the pinned callers and a repair action to inspect dependents.
- Added Rust integration coverage for successful standalone version bumps and rejected pinned-call version bumps.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document pinned-call-aware version patching.
- Added `bin/serow query callees <symbol-or-name> [paths...] [--json]` to report direct outgoing call edges for a public symbol.
- Callee query rows include caller, callee, source/version metadata, and resolved implementation/contract/example/property call sites, mirroring the existing direct dependent ledger style in the forward direction.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to advertise the forward-call ledger query.
- Added `bin/serow patch set-impl <path> <symbol-or-name> <expression> [--json]`.
- The set-impl patch replaces an existing implementation expression through the structured patch interface, rejects empty expressions and functions without implementation sections, preserves ambiguous-target protection, and rewrites through the canonical formatter.
- Added Rust integration coverage proving `patch set-impl` replaces an implementation while leaving the result checkable.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to advertise structured implementation replacement.
- Expanded duplicate-intent diagnostics so both exact `PossibleDuplicate` errors and `NearDuplicateIntent` warnings include structured `shared_terms`, `new_only_terms`, and `candidate_only_terms` data.
- Exposed canonical intent-token extraction in the Rust ledger and Python reference bootstrap so reuse diagnostics use the same stopword filtering and light token normalization as intent search, with a raw normalized-word fallback for very short exact intents.
- Added Rust and Python regression coverage proving duplicate and near-duplicate intent diagnostics report actionable overlap and difference data.
- Added `bin/serow patch set-intent <path> <symbol-or-name> <intent> [--json]`.
- The set-intent patch sets or replaces a function intent through the structured patch interface, rejects empty intents, preserves ambiguous-target protection, and rewrites through the canonical formatter.
- Updated command discovery docs, `serow.project`, and Progress notes to advertise structured intent replacement.
- Added `bin/serow patch set-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]`.
- The initial set-contract patch created a missing `requires` or `ensures` clause or replaced a single existing clause, rejected empty expressions and invalid clause names, preserved ambiguous-target protection, and rejected multi-clause replacements until Serow had indexed contract patching.
- Added Rust integration coverage proving `patch set-contract` can repair a wrong postcondition, add a missing precondition clause, and reject ambiguous multi-clause replacement.
- Extended `bin/serow patch set-contract` with an optional 1-based clause index before the expression.
- Indexed contract patches now replace a specific existing `requires` or `ensures` clause while preserving the older missing/single-clause behavior.
- Added Rust integration coverage for indexed contract replacement and out-of-range index diagnostics.
- Updated command discovery docs, `serow.project`, README, and Progress notes to advertise structured contract clause replacement.

## 2026-05-06

- Added `bin/serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]`.
- Added `bin/serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]`.
- The new evidence setters create missing executable evidence, replace a single existing item, or replace a specific item with a 1-based index, while preserving ambiguous-target protection and canonical formatting.
- Added Rust integration coverage for single evidence replacement, indexed example/property replacement, multi-evidence ambiguity, and out-of-range property indexes.
- Updated command discovery docs, `serow.project`, README, and Progress notes to advertise structured example and property replacement.
- Added `bin/serow patch rename-function <path> <symbol-or-name> <new-name> [--json]`.
- The rename patch validates the new function name, rejects duplicate canonical symbols, renames the public declaration, and rewrites resolved call references in implementations, contracts, examples, and properties in the patched source.
- Rewritten bare calls use the exact `@module.name.vN(...)` form when the requested new bare name would collide with another public function and become ambiguous.
- Added Rust integration coverage for normal function renames and collision-aware exact call rewriting.

## 2026-05-12

- Added spec-quality repeated-evidence diagnostics to the Rust checker and Python reference checker.
- `DuplicateExample` now warns when a public function repeats an executable example exactly after whitespace normalization.
- `DuplicateContractClause` now warns when a function repeats a `requires` or `ensures` clause.
- `DuplicateProperty` now warns when a function repeats the same sampled `forall` property block.
- Added Rust and Python regression coverage proving repeated evidence produces warnings without hiding otherwise passing behavior.
- Updated README, `serow.project`, agent bootstrap output, and Progress docs to document low-signal duplicate evidence warnings.
- Added deterministic replay data to sampled property failure diagnostics in the Rust checker and Python reference checker.
- `PropertyFailed` and `PropertyEvaluationError` now include `property_index`, `sample_index`, `sample_seed`, and sampled `bindings`.
- Added Rust and Python regression coverage proving failing sampled properties expose replay data.
- Added low-signal shallow property diagnostics to the Rust checker and Python reference checker.
- `ShallowProperty` now warns when a sampled property for a public function does not directly call the function under test, making tautological or unrelated properties visible before certification.
- Added Rust and Python regression coverage proving shallow sampled properties report their property index and expression.
- Added low-signal vacuous property diagnostics to the Rust checker and Python reference checker.
- `VacuousProperty` now warns when a sampled `forall` block binds no variables and is only checked once, making example-like properties visible before certification.
- Added Rust and Python regression coverage proving vacuous sampled properties report their property index and expression.
- Added `bin/serow replay property <sample-seed> [paths...] [--json]` to rerun a single sampled property binding from deterministic replay data.
- `PropertyFailed` and `PropertyEvaluationError` diagnostics now include a structured command repair action for property replay.
- Added Rust regression coverage proving replay diagnostics expose the command action and the replay CLI returns the same binding and actual result.
- Centralized the Rust bootstrap's sampled property value generation so checker execution, property replay, and change-plan HEAD-sensitivity analysis use the same sample sets.
- Expanded built-in sampled property values while preserving the original first samples for stable replay seeds: `Int` now also samples `-10` and `10`, and `Text` now also samples spaced and numeric-looking strings.
- Updated the Python reference checker and added Rust/Python regression coverage proving the expanded `Int` sample set finds a larger counterexample and replays the same sample binding.
- Added deterministic shrinking metadata for failing sampled properties in the Rust checker and Python reference checker.
- `PropertyFailed` diagnostics now include `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` when a simpler failing binding exists within the built-in sample set.
- Added direct-call capability analysis to `bin/serow plan`.
- Changed-symbol plan rows now include `capability_analysis` with declared effects, declared concrete capabilities, inferred non-self direct-callee capability requirements, missing direct-call capabilities, unused wrapper capabilities, and a suggested effect declaration.
- Human-readable plan output now summarizes the same direct-call capability analysis for each changed symbol.
- Added Rust integration coverage proving plan JSON reports both missing and unused direct-call capabilities with a deterministic suggested declaration.
- Added semantic change labels to `bin/serow plan`.
- Changed-symbol plan rows now include `semantic_changes` with deterministic labels, acknowledgement state, and details for public deltas such as evidence weakening, capability expansion, implementation changes, implementation evidence sensitivity, impacted dependents, uncovered impact evidence, and direct-call capability declaration issues.
- Human-readable plan output now summarizes semantic change labels for changed symbols.
- Added Rust integration assertions proving plan JSON reports semantic labels for evidence weakening, implementation changes, and capability expansion.
- Added advisory intent/implementation mismatch risks to `bin/serow plan`.
- Changed-symbol plan rows now include `intent_implementation_risks` when a function name or intent clearly indicates an arithmetic operation but the implementation uses a conflicting operator or does not use the expected operator/helper.
- The advisory also appears as a `intent_implementation_mismatch_risk` semantic change label and a residual plan risk, but it is not a checker or certification gate.
- Added Rust integration coverage proving a function whose intent says arithmetic sum while its implementation subtracts is reported by plan JSON.
- Added sampled-property coverage hints to `bin/serow plan`.
- Changed-symbol plan rows now include `property_coverage` with per-property sample counts, direct-call flags, vacuous flags, unsupported generator types, variables, and normalized body expressions.
- Human-readable plan output now summarizes sampled-property coverage hints for changed symbols.
- Added Rust integration assertions proving plan JSON reports sampled-property coverage data.
- Extended `bin/serow patch set-impl <path> <symbol-or-name> <expression> [--json]` so it can create a missing implementation section as well as replace an existing implementation expression.
- Added Rust integration coverage proving `patch set-impl` repairs a function missing only its `impl` section and leaves the result checkable.
- Added indexed evidence-removal structured patches:
  - `bin/serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]`
  - `bin/serow patch remove-example <path> <symbol-or-name> <index> [--json]`
  - `bin/serow patch remove-property <path> <symbol-or-name> <index> [--json]`
- Duplicate public evidence diagnostics now attach command-style repair actions pointing at the repeated evidence item.
- Added Rust integration coverage proving indexed evidence removal rewrites canonical source and duplicate-evidence diagnostics expose the new repair commands.
- Added `bin/serow patch set-signature <path> <symbol-or-name> <signature> [--json]`.
- The signature patch replaces a function's argument list and return type while rejecting invalid signatures and signatures whose name differs from the target function, leaving renames to `patch rename-function`.
- Updated command discovery docs, `serow.project`, README, and Progress notes to advertise structured signature replacement.
- Added Rust integration coverage proving `patch set-signature` rewrites the canonical function header and rejects accidental renames.
- Added structured implementation-obligation data to `TypedHole` diagnostics in the Rust checker and Python reference checker.
- Hole diagnostics now report the public symbol, signature, hole type, expected return type, and obligations derived from return type, preconditions, postconditions, examples, and sampled properties.
- Added Rust and Python regression coverage proving typed-hole diagnostics expose those obligations.
- Verified with `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `bin/serow agent --json`.
- Preserved deterministic shrinking metadata through single-sample property replay.
- `bin/serow replay property` now includes `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` in replayed `PropertyFailed` diagnostics when a simpler failing sampled binding exists.
- Added Rust regression coverage proving replay JSON carries the same shrink hint fields as the original checker diagnostic.
- Added `bin/serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]`.
- The migration setter creates a missing acknowledgement for a kind, replaces a single existing record of that kind, or replaces a specific same-kind record by 1-based index while preserving ambiguous-target protection and canonical formatting.
- Updated agent bootstrap output, repair-action validation, README, `serow.project`, and Progress notes to advertise structured migration acknowledgement replacement.
- Added structured remove-property repair actions to `VacuousProperty` and `ShallowProperty` diagnostics in the Rust checker.
- Low-signal sampled-property warnings now point at the exact indexed `bin/serow patch remove-property` command for removing the vacuous or shallow property.
- Added `bin/serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]`.
- The migration remover deletes one same-kind migration acknowledgement by 1-based index, rejects unsupported migration kinds, preserves ambiguous-target protection, and rewrites through canonical formatting.
- Updated command discovery docs, repair-action validation, README, `serow.project`, and Progress notes to advertise structured migration acknowledgement removal.
- Verified with `cargo fmt --check`, `cargo test patch_remove_migration_removes_indexed_same_kind_records -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `bin/serow agent --json`.

## 2026-05-13

- Added `bin/serow patch remove-use <path> <module> <dependency> [--json]`.
- The module dependency remover validates module names, rejects unknown modules or missing dependency declarations, removes the existing `use` record, and rewrites through canonical formatting.
- Updated command discovery docs, repair-action validation, README, `serow.project`, and Progress notes to advertise structured module dependency removal.
- Verified with `cargo fmt --check`, `cargo test patch_remove_use_updates_source -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added structured `patch remove-use` repair actions to declared `ArchitectureViolation` diagnostics for forbidden module dependency declarations.
- The architecture-policy regression now verifies the JSON repair action and applies it end to end before rechecking the source.
- Updated the agent bootstrap diagnostic contract to document exact add-use/remove-use architecture repair actions.
- Verified with `bin/serow query intent "remove forbidden module dependency declaration" --json`, `bin/serow query symbol remove-use --json`, `cargo test architecture_policy_rejects_disallowed_use -- --nocapture`, `cargo test agent_json_includes_machine_readable_workflow -- --nocapture`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added structured `query symbol` repair actions to `AmbiguousUnqualifiedCall` diagnostics.
- Ambiguous bare-call diagnostics now expose candidate canonical symbols and a command action for inspecting the candidate set before rewriting the call with a qualified reference.
- Updated the agent bootstrap diagnostic contract, README, `serow.project`, and Progress notes to advertise ambiguous-call symbol lookup repair actions.
- Verified with `bin/serow query intent "repair ambiguous unqualified call diagnostics" --json`, `bin/serow query symbol "AmbiguousUnqualifiedCall" --json`, `cargo fmt --check`, `cargo test ambiguous_unqualified_calls_are_reported -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added conservative structured repair actions to `MissingRequiredSection` diagnostics for absent non-evidence sections.
- Missing effects now point to `patch set-effects ... pure` as an explicit baseline, and missing implementations point to `patch set-impl ... HOLE(Type)` so agents can create a typed hole without inventing behavior.
- Verified with `cargo fmt --check`, `cargo test missing_required_sections_include_safe_patch_repair_actions -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added Python reference bootstrap support for serializing machine-readable `repair_actions`.
- Mirrored Rust's safe `MissingRequiredSection` repair actions in the Python checker for absent `effects` and `impl` sections, emitting exact `patch set-effects ... pure` and `patch set-impl ... HOLE(Type)` command actions.
- Added Python regression coverage for the missing-section repair-action payload.
- Verified with `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_missing_required_sections_include_structured_repair_actions`, `python3 -m unittest discover -s tests`, `cargo fmt --check`, `cargo test missing_required_sections_include_safe_patch_repair_actions -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Mirrored Rust's indexed evidence-removal repair actions in the Python reference checker for duplicate examples, duplicate contract clauses, duplicate sampled properties, vacuous sampled properties, and shallow sampled properties.
- Added Python regression coverage proving those warnings carry the expected `patch remove-example`, `patch remove-contract`, and `patch remove-property` command payloads.
- Verified with `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_repeated_public_evidence_is_warned tests.test_bootstrap.BootstrapTests.test_sampled_property_without_target_call_warns_as_shallow tests.test_bootstrap.BootstrapTests.test_sampled_property_without_bindings_warns_as_vacuous`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added exact duplicate-intent protection to `bin/serow patch add-function`.
- The public function skeleton patch now uses the ledger's normalized intent key and rejects a new skeleton before writing when an existing public function in the parsed patch input already has that intent.
- Reused the existing `PossibleDuplicate` diagnostic shape with a `query intent` repair action so agents can inspect and reuse existing behavior before creating a new public symbol.
- Verified with `bin/serow query intent "reject duplicate public function intents during structured function insertion" --json`, `bin/serow query symbol add-function --json`, `cargo fmt --check`, `cargo test add_function -- --nocapture`, `cargo test agent_json_includes_machine_readable_workflow -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `bin/serow agent --json`.
- Added low-signal executable-example diagnostics to the Rust checker and Python reference checker.
- `ShallowExample` now warns when an executable example for a public function does not directly call the function under test, and includes an indexed `patch remove-example` structured repair action.
- Updated agent bootstrap output, README, `serow.project`, and Progress docs to document shallow executable-example warnings.
- Verified with `bin/serow query intent "detect executable examples that do not call the function under test" --json`, `bin/serow query symbol ShallowExample --json`, `cargo fmt --check`, `cargo test executable_example_without_target_call_warns_as_shallow -- --nocapture`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_executable_example_without_target_call_warns_as_shallow`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added stale migration acknowledgement analysis to `bin/serow plan`.
- Changed-symbol plan rows now include `stale_migrations` with same-kind migration indexes, notes, and reasons when a migration record remains but no current unattended gate requires that acknowledgement kind.
- Added a `stale_migration_acknowledgement` semantic change label and residual risk reporting so stale acknowledgements are visible without parsing migration text.
- Added strict unattended certification diagnostics for stale acknowledgements as `StaleMigrationAcknowledgement`, with structured `patch remove-migration` repair actions targeting the exact same-kind index.
- Updated agent bootstrap output, README, `serow.project`, and Progress docs to advertise stale migration plan rows and the strict-profile gate.
- Verified with `bin/serow query intent "detect stale migration acknowledgements" --json`, `bin/serow query symbol remove-migration --json`, `cargo fmt --check`, `cargo test stale_migration_acknowledgement_is_reported_and_rejected -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added indexed structured repair actions to non-executable sampled-property diagnostics.
- `PropertyNotExecutable` now reports the property index, unsupported sampled types, and an exact `patch remove-property` command for removing the non-executable property evidence.
- Mirrored the same diagnostic data and repair action in the Python reference checker.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document non-executable property removal actions.
- Verified with `bin/serow query intent "repair unsupported sampled property diagnostics" --json`, `bin/serow query symbol PropertyNotExecutable --json`, `cargo test sampled_property_with_unsupported_type_has_indexed_repair_action -- --nocapture`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_with_unsupported_type_has_indexed_repair_action`, `cargo fmt --check`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, and `bin/serow plan --json`.
- Added duplicate migration acknowledgement diagnostics to the Rust checker and Python reference checker.
- `DuplicateMigration` now warns when a public function repeats the same migration kind and note, indexes duplicates within the migration kind, and includes an exact `patch remove-migration` repair action for the repeated acknowledgement.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document duplicate migration removal actions.
- Verified with `bin/serow query intent "detect duplicate migration acknowledgements" --json`, `bin/serow query symbol DuplicateMigration --json`, `bin/serow check --json`, `cargo test repeated_public_migrations_are_warned -- --nocapture`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_repeated_public_migrations_are_warned`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added `bin/serow query type <type-or-shape> [paths...] [--json]`.
- Type queries rank public functions by declared signature shape, accepting exact forms such as `Int, Int -> Int`, wildcard forms such as `_ -> Int`, and simple type-token queries such as `Text`.
- Updated the agent bootstrap contract, README, and Progress docs to advertise type-shape ledger lookup.
- Verified with `bin/serow query intent "find public functions by type signature" --json`, `bin/serow query symbol "query type" --json`, `cargo fmt --check`, `cargo test type_query_finds_functions_by_signature_shape -- --nocapture`, `cargo test agent_json_includes_machine_readable_workflow -- --nocapture`, `bin/serow query type "Int, Int -> Int" --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Extended strict repair-action contract validation to accept `bin/serow query type ...` command actions now that type-shape lookup is a public ledger query.
- Added `query type` repair actions to Rust and Python `TypedHole` diagnostics.
- Typed-hole diagnostics now point agents at public functions with the same declared signature shape before the implementation hole is filled.
- Verified with `bin/serow query intent "suggest reusable functions for typed implementation holes by type" --json`, `bin/serow query symbol TypedHole --json`, `cargo fmt --check`, `cargo test typed_hole_reports_structured_obligations -- --nocapture`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_typed_hole_reports_structured_obligations`, `cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added Rust regression coverage proving synthetic type-lookup repair actions pass the same validator used by unattended certification.
- Added structured symbol-lookup repair actions to Rust static `TypeError` diagnostics for unknown function calls.
- Unknown function type errors now include `unknown_function` data and an exact `bin/serow query symbol <name> <path>` command action so agents can check for typos or reusable symbols before inventing new behavior.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document unknown-function lookup repairs.
- Verified with `bin/serow query intent "repair unknown function references with symbol lookup" --json`, `bin/serow query symbol TypeError --json`, `bin/serow query symbol UnknownFunction --json`, `cargo fmt --check`, `cargo test unknown_function_type_errors_include_symbol_lookup_repair -- --nocapture`, `cargo test agent_json_includes_machine_readable_workflow -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added removed public symbol reporting to `bin/serow plan`.
- Change plans now compare changed tracked files against `HEAD` for canonical public symbols that disappeared, report same-name replacement candidates for version bumps, and add residual risk when a public symbol is removed without a same-name replacement version.
- `bin/serow certify --profile unattended` now rejects unversioned public symbol removal as `PublicSymbolRemoved`, with a structured repair action pointing back to `bin/serow plan --json`.
- Updated command discovery docs, README, `serow.project`, and Progress notes to advertise removed-public-symbol plan rows and the strict-profile gate.
- Verified with `bin/serow query intent "detect removed public symbols in change plans" --json`, `bin/serow query symbol PublicSymbolRemoved --json`, `cargo fmt --check`, `cargo test plan_and_unattended_certification_report_removed_public_symbols -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added deterministic shrinking metadata to sampled property evaluation-error diagnostics.
- `PropertyEvaluationError` now includes `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` when another built-in sample produces an evaluation error with lower sample complexity.
- `bin/serow replay property` preserves the same shrink hint fields when replaying an erroring sample, and the Python reference checker now mirrors the checker-side diagnostic data.
- Verified with `bin/serow query intent "shrink property evaluation error samples" --json`, `bin/serow query symbol PropertyEvaluationError --json`, `cargo test sampled_property_evaluation_error_reports_shrunk_replay_data -- --nocapture`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_evaluation_error_reports_shrunk_data`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Mirrored Rust's structured ambiguous-call lookup repair action in the Python reference checker.
- Python `AmbiguousUnqualifiedCall` diagnostics now include an exact `bin/serow query symbol <call> <path>` command action so reference-checker consumers can inspect candidate symbols before qualifying a call.
- Verified with `bin/serow query intent "mirror ambiguous unqualified call repair actions in the Python reference checker" --json`, `bin/serow query symbol "AmbiguousUnqualifiedCall" --json`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_ambiguous_unqualified_calls_are_reported`, `python3 -m unittest discover -s tests`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added exact duplicate-intent protection to `bin/serow patch set-intent`.
- The intent setter now rejects an intent replacement before writing when another public function in the parsed patch input has the same exact normalized intent, returning the same `PossibleDuplicate` diagnostic and `query intent` repair action used by `patch add-function`.
- Updated the agent bootstrap contract, README, `serow.project`, and Progress docs to document duplicate-intent protection for structured intent replacement.
- Verified with `bin/serow query intent "prevent structured intent replacement from creating duplicate public intents" --json`, `bin/serow query symbol set-intent --json`, `cargo fmt --check`, `cargo test patch_set_intent_rejects_duplicate_public_intent -- --nocapture`, `cargo test agent_json_includes_machine_readable_workflow -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Added `bin/serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]`.
- The call qualifier rewrites bare calls inside one caller function to an exact selected callee symbol, rejects invalid bare call names, unknown or ambiguous caller/callee targets, callee-name mismatches, and caller functions without a matching bare call.
- Updated command discovery docs, repair-action validation, README, `serow.project`, and Progress notes to advertise structured call qualification as the patch follow-up after ambiguous-call symbol lookup.
- Verified with `bin/serow query intent "qualify ambiguous bare function calls through structured patches" --json`, `bin/serow query symbol qualify-call --json`, `bin/serow query symbol AmbiguousUnqualifiedCall --json`, `cargo fmt --check`, `cargo test patch_qualify_call_rewrites_bare_calls_to_exact_symbol -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.
- Mirrored Rust's sampled-property replay repair actions in the Python reference checker.
- Python `PropertyFailed` and `PropertyEvaluationError` diagnostics now include an exact `bin/serow replay property <sample-seed> <path>` command action alongside deterministic sample and shrink data.
- Verified with `bin/serow query intent "mirror property replay repair actions in the Python reference checker" --json`, `bin/serow query symbol PropertyFailed --json`, and `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_failure_reports_replay_data tests.test_bootstrap.BootstrapTests.test_sampled_property_evaluation_error_reports_shrunk_data`.
- Mirrored Rust's effect capability declaration repair actions in the Python reference checker.
- Python `EffectViolation` diagnostics now include an exact `bin/serow patch set-effects ...` command with the union of already-declared and missing required capabilities, and `UnusedEffectCapability` diagnostics include the exact declaration needed by resolved non-self direct callees.
- Verified with `bin/serow query intent "mirror effect capability repair actions in the Python reference checker" --json`, `bin/serow query symbol EffectViolation --json`, and `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_effectful_function_must_declare_specific_called_capabilities`.
- Added indexed `patch remove-property` repair actions to `bin/serow replay property` diagnostics for non-executable sampled properties.
- Replay-side `PropertyNotExecutable` diagnostics now report `function`, `property_index`, `property`, and `unsupported_types`, matching the checker-side action protocol for unsupported generator types.
- Verified with `bin/serow query intent "remove low signal duplicate evidence through structured patch repairs" --json`, `bin/serow query symbol replay --json`, `cargo fmt --check`, and `cargo test property_replay_unsupported_type_has_indexed_repair_action -- --nocapture`.
- Mirrored unknown-function symbol lookup repair actions in the Python reference checker for runtime evaluation diagnostics.
- Python `ExampleError`, `ContractEvaluationError`, and `PropertyEvaluationError` diagnostics caused by `Unknown function` failures now include `unknown_function` data and an exact `bin/serow query symbol <name> <path>` command action.
- Updated `serow.project` and Progress notes to record the Python reference diagnostic behavior.
- Verified with `bin/serow query intent "repair unknown function references with symbol lookup" --json`, `bin/serow query symbol TypeError --json`, `bin/serow query symbol UnknownFunction --json`, `python3 -m unittest tests.test_bootstrap.BootstrapTests.test_unknown_function_evaluation_errors_include_symbol_lookup_repair`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, and `git diff --check`.

## 2026-05-13

- Refactored `bin/serow agent --json` into a compact bootstrap contract for AI bootstrap consumption.
- Added `bin/serow agent commands [--json]` for the full command catalog and `bin/serow agent diagnostics [--json]` for detailed diagnostic and plan JSON protocol notes.
- Updated usage text, README, `serow.project`, and Progress docs to describe the split between compact bootstrap data and explicit reference material.
- Verified with `bin/serow query intent "agent json compact default commands diagnostics subcommands"`, `bin/serow query symbol "agent"`, `cargo test agent_ -- --nocapture`, `bin/serow agent --json`, `bin/serow agent commands --json`, `bin/serow agent diagnostics --json`, `bin/serow agent commands`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, and `bin/serow plan --json`.
- Started Phase 3 backend work with a portable IR foundation.
- Added `src/ir.rs` with a `serow.ir.v0` lowering model for the checked bootstrap expression subset.
- Added `bin/serow compile ir [paths...] [--json]`.
- The command parses source paths, runs the normal checker, refuses to emit IR when checker errors are present, and emits public function rows with canonical symbol identity, signature, declared effects, lowered implementation expression trees, and resolved canonical call targets.
- Updated `bin/serow agent`, README, `serow.project`, and Progress docs to advertise the Phase 3 IR command while keeping generated Rust/backend artifacts explicitly out of scope for this slice.
- Added Rust CLI regression coverage for successful IR JSON output and checker-error refusal.
- Verified with `bin/serow query intent "emit portable intermediate representation for Serow functions" --json`, `bin/serow query symbol "ir" --json`, `bin/serow query symbol "backend" --json`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test compile_ir -- --nocapture`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile ir examples/math.serow`, and `git diff --check`.
- Added the first Rust backend emitter: `bin/serow compile rust [paths...] [--json]`.
- The Rust backend reuses checked `serow.ir.v0` lowering, emits deterministic Rust source for pure `Int`/`Bool` functions, maps canonical symbols to stable Rust function names, and preserves resolved call targets without repeating source-level name resolution.
- The backend currently rejects unsupported `Text` lowering and non-`pure` effects with explicit backend diagnostics instead of generating partial Rust.
- Plain `compile rust` prints generated Rust source to stdout; JSON output includes backend id `serow.rust.v0`, input IR version, generated source, and symbol-to-Rust-name rows.
- Updated `bin/serow agent`, README, `serow.project`, and Progress docs to advertise the Rust backend slice and its current limits.
- Added Rust CLI regression coverage for successful Rust JSON output and unsupported `Text` diagnostics.
- Verified with `bin/serow query intent "generate Rust backend artifact from checked Serow IR" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "Rust backend" --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `bin/serow compile rust examples/math.serow > /private/tmp/serow_math_generated.rs`, `rustc --crate-type lib /private/tmp/serow_math_generated.rs -o /private/tmp/libserow_math_generated.rlib`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.
- Changed the recorded implementation mode from phase-only selection to cross-phase implementation.
- Future generic implementation prompts now have a documented selection policy: inspect unfinished, deferred, and known-limit items across all phases, choose the highest-leverage next step toward completing Serow, and record the chosen focus in Progress.
- Kept Phase 3 backends as the current advanced track, but explicitly allowed returning to earlier-phase gaps when they are higher leverage, block later work, or are required before Serow can be considered complete.
- Updated `bin/serow agent`, README, `serow.project`, and Progress docs so this policy is discoverable from both markdown and the agent bootstrap command.
- Expanded the Rust backend to lower pure `Text` functions.
- `bin/serow compile rust` now maps Serow `Text` to owned Rust `String` values, escapes text literals into Rust string literals, clones text parameters when rendering expressions, and emits text concatenation with `format!`.
- Added Rust regression coverage that compiles generated `Text` backend source with `rustc`.
- Verified with `bin/serow query intent "generate Rust backend artifact for Text functions" --json`, `bin/serow query symbol "Text" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query type "Text -> Text" --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.
- Chose backend artifact layout as the next cross-phase task because `serow compile rust` already had checked source generation but still required shell redirection and hand-created crate scaffolding.
- Added `bin/serow compile rust [paths...] --out-dir <dir> [--json]`.
- The new mode reuses checked Rust backend generation and writes a dependency-free Rust crate layout with `Cargo.toml` and `src/lib.rs`; JSON output now includes a deterministic `written_files` list.
- Added Rust regression coverage that generates a crate from `examples/math.serow` and runs `cargo check` against the generated manifest.
- Verified with `bin/serow query intent "write generated Rust crate layout from checked Serow backend" --json`, `bin/serow query symbol "compile rust" --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir <tmpdir> --json`, `cargo check --manifest-path <tmpdir>/Cargo.toml`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent commands --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.

## 2026-05-16

- Chose Rust backend precondition preservation as the next cross-phase task because checked Serow calls enforce `requires` clauses but generated Rust functions previously exposed raw implementation bodies.
- Extended `serow.ir.v0` function rows with lowered `requires` precondition expressions alongside implementation bodies.
- Updated `bin/serow compile rust` to emit runtime `assert!` guards for each Serow precondition before the generated function body.
- Updated backend docs, agent bootstrap known limits, `serow.project`, and Progress notes to describe IR preconditions and Rust precondition assertions.
- Verified with `bin/serow query intent "preserve requires preconditions in generated Rust backend functions" --json`, `bin/serow query symbol "requires" --json`, `bin/serow query symbol "RustBackend" --json`, `bin/serow query symbol "compile rust" --json`, `cargo fmt --check`, `cargo test compile_ -- --nocapture`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, generated-crate `cargo check`, and `git diff --check`.
- Chose generated backend example tests as the next cross-phase task because Serow examples were checked before codegen but generated Rust crates did not carry executable evidence with them.
- Extended `serow.ir.v0` function rows with lowered executable examples alongside lowered implementations and `requires` preconditions.
- Updated `bin/serow compile rust` to emit one Rust `#[test]` function per checked Serow example, plus JSON symbol/example-to-test mappings and a generated-test summary count.
- Updated backend docs, agent bootstrap known limits, `serow.project`, and Progress notes to describe IR examples and generated Rust tests.
- Verified with `bin/serow query intent "emit generated Rust tests from Serow examples" --json`, `bin/serow query symbol example --json`, `cargo fmt --check`, `cargo test compile_ -- --nocapture`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, generated-crate `cargo test`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, and `git diff --check`.
- Chose generated backend postcondition preservation as the next cross-phase task because the IR and Rust backend already preserved `requires` preconditions and examples but did not carry checked `ensures` contracts into generated artifacts.
- Extended `serow.ir.v0` function rows with lowered `ensures` postcondition expressions, binding `result` explicitly for contract lowering.
- Updated `bin/serow compile rust` to evaluate each function body into a deterministic result local, assert every lowered `ensures` postcondition against that result, and then return the stored value.
- Updated backend docs, agent bootstrap known limits, `serow.project`, README, and Progress notes to describe postcondition-preserving IR and generated Rust runtime assertions.
- Verified with `bin/serow query intent "preserve ensures contracts in generated Rust backend functions" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "ensures" --json`, `cargo fmt --check`, `cargo test compile_ -- --nocapture`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, generated-crate `cargo test`, `bin/serow agent --json`, `bin/serow agent commands --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.
- Chose generated backend sampled-property preservation as the next cross-phase task because generated crates carried checked examples and contracts but not the sampled property evidence Serow already evaluates before codegen.
- Extended `serow.ir.v0` function rows with lowered sampled properties, including property indexes, forall bindings, and resolved expression trees.
- Updated `bin/serow compile rust` to emit one deterministic Rust `#[test]` per sampled-property binding for supported `Int`, `Bool`, and `Text` sample types, and to report property test mappings in JSON mode alongside example tests.
- Updated backend docs, agent bootstrap known limits, `serow.project`, README, and Progress notes to describe sampled-property-preserving IR and generated Rust property tests.
- Verified with `bin/serow query intent "emit generated Rust tests from sampled properties" --json`, `bin/serow query symbol property --json`, `bin/serow query symbol "compile rust" --json`, `cargo fmt --check`, `cargo test compile_ -- --nocapture`, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, generated-crate `cargo test`, `bin/serow agent --json`, `bin/serow agent commands --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.
- Chose configurable generated Rust crate naming as the next cross-phase task because `compile rust --out-dir` could write a crate layout but always forced the package name to `serow_generated`.
- Added `bin/serow compile rust [paths...] --out-dir <dir> --crate-name <name> [--json]` with conservative lowercase ASCII Cargo package-name validation; the default remains `serow_generated`.
- Updated generated `Cargo.toml` rendering, JSON summaries, command discovery, README, `serow.project`, and Progress notes to report the selected crate name.
- Added Rust integration coverage for custom crate-name output and invalid crate-name rejection.
- Verified with `bin/serow query intent "configure generated Rust crate package metadata" --json`, `bin/serow query symbol crate --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `cargo test agent_commands_json_includes_full_command_catalog -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, generated-crate `cargo test` with `--crate-name serow_math`, invalid crate-name CLI rejection, and `git diff --check`.
- Chose generated Rust crate metadata as the next cross-phase task because generated crates had stable source and tests but no machine-readable link back to Serow backend identity or public symbols in the manifest.
- Updated `bin/serow compile rust --out-dir` to write deterministic `package.metadata.serow` rows in `Cargo.toml`, including backend id, IR version, generated function/test counts, and symbol-to-Rust-name mappings.
- Updated command discovery, README, `serow.project`, and Progress docs to advertise the generated manifest metadata and narrow the remaining backend package-layout limits.
- Verified with `bin/serow query intent "include Serow source metadata in generated Rust backend crate artifacts" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "Rust backend" --json`, `cargo fmt --check`, `cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow agent commands --json`, `bin/serow compile rust examples/math.serow --json`, generated-crate `cargo test` with `--crate-name serow_math`, and `git diff --check`.
- Chose generated Rust evidence metadata as the next cross-phase task because generated crates recorded function mappings but not the example/property provenance of generated Rust tests.
- Extended generated `Cargo.toml` metadata with deterministic `[[package.metadata.serow.tests]]` rows, including Serow symbol, evidence kind, Rust test name, example indexes, property indexes, and sampled-property sample indexes.
- Updated Rust backend JSON test rows to include `kind` for generated example tests as well as property tests.
- Updated README, `serow.project`, and Progress docs to advertise example/property evidence-to-test metadata in generated crates.
- Verified with `bin/serow query intent "record generated Rust backend evidence test metadata" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "Rust backend" --json`, `bin/serow check --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `git diff --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow compile rust examples/math.serow --json`, `bin/serow agent commands --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and generated-crate `cargo test` with `--crate-name serow_math`.
- Chose compact agent backend discoverability as the next cross-phase task because `bin/serow agent --json` advertised the Phase 3 IR command but omitted the implemented Rust backend command from the primary bootstrap command list.
- Added `compile rust` to the compact `agent` command set while keeping structured patch commands and verbose protocol details in `agent commands` and `agent diagnostics`.
- Updated `serow.project` to version `0.4.68-rust-bootstrap` and added regression coverage proving the compact JSON bootstrap now exposes the Rust backend usage.
- Verified with `bin/serow query intent "compile Serow programs to Rust crate and run generated evidence tests" --json`, `bin/serow query symbol compile --json`, `cargo fmt --check`, `cargo test agent_json_includes_compact_machine_readable_workflow -- --nocapture`, `bin/serow agent --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.
- Chose IR and backend source provenance as the next cross-phase task because generated Rust artifacts recorded canonical symbols and evidence mappings but not the original source path and line for each generated function.
- Extended `serow.ir.v0` function rows with `source_path` and `line` fields.
- Propagated function source provenance into `bin/serow compile rust --json` function mappings and generated crate `package.metadata.serow.functions` manifest rows.
- Updated README, `serow.project`, language notes, and current progress state to advertise source-location metadata in IR and generated Rust artifacts.
- Verified with `bin/serow query intent "record source file and line provenance in portable IR and generated Rust backend metadata" --json`, `bin/serow query symbol "source_path" --json`, `bin/serow query symbol "compile rust" --json`, `cargo fmt --check`, targeted `cargo test` for IR/Rust backend metadata, `bin/serow compile ir examples/math.serow --json`, `bin/serow compile rust examples/math.serow --json`, `bin/serow fmt --check --json`, `bin/serow check --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and generated-crate `cargo test` with `--crate-name serow_math`.
- Chose generated Rust evidence-test provenance as the next cross-phase task because generated backend test mappings identified evidence indexes but not the source file and line for the Serow function that produced each test.
- Extended Rust backend test rows with Serow source path and line provenance, and propagated those fields into `compile rust --json` output plus generated crate `package.metadata.serow.tests` manifest rows.
- Updated README, `serow.project`, language notes, and current progress state to advertise source-location-aware evidence-to-test metadata.
- Verified with `bin/serow query intent "record source provenance for generated Rust backend evidence tests" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "GeneratedRustTest" --json`, `cargo fmt --check`, targeted Rust backend tests, `bin/serow compile rust examples/math.serow --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and generated-crate `cargo test` with `--crate-name serow_math`.
- Chose generated Rust source fingerprint metadata as the next cross-phase task because generated crates recorded provenance mappings but had no deterministic value tying the generated `src/lib.rs` contents to the JSON and manifest metadata.
- Added a stable `fnv1a64:<hex>` generated-source fingerprint to Rust backend summaries and generated `package.metadata.serow`.
- Updated README, `serow.project`, language notes, and current progress state to advertise generated-source fingerprint metadata.
- Verified with `bin/serow query intent "record deterministic fingerprint for generated Rust backend artifacts" --json`, `bin/serow query symbol "compile rust" --json`, `bin/serow query symbol "Rust backend" --json`, `cargo fmt --check`, `cargo test compile_rust -- --nocapture`, `bin/serow compile rust examples/math.serow --json`, generated-crate `cargo test` with `--crate-name serow_math`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, and `bin/serow plan --json`.
- Chose runnable Rust binary crate emission as the next backend slice because generated Rust crates could run evidence tests but had no pure application entrypoint convention.
- Added `bin/serow compile rust [paths...] --out-dir <dir> --emit-bin [--json]`, with `--bin` as an alias. Binary emission validates exactly one public zero-argument `main` returning `Text`, `Int`, or `Bool`, writes `src/main.rs`, calls the generated Serow entrypoint, and prints the returned value deterministically.
- Added explicit diagnostics for invalid binary entrypoints: `RustBinaryMissingEntrypoint`, `RustBinaryEntrypointArity`, `RustBinaryUnsupportedEntrypointReturn`, `RustBinaryAmbiguousEntrypoint`, and `RustBinaryEntrypointNotGenerated`.
- Updated generated crate metadata with optional binary entrypoint rows, refreshed CLI/agent command usage, README/backend docs, `serow.project`, and Progress notes.
- Verified with `bin/serow query intent "runnable Rust binary entrypoint compile rust emit binary crate main function"`, `bin/serow query symbol "main"`, `cargo fmt --check`, `cargo test compile_rust_emit_bin -- --nocapture`, `cargo test compile_rust -- --nocapture`, `bin/serow compile rust examples/math.serow --out-dir /private/tmp/serow-no-main-bin --emit-bin --json`, `cargo clippy -- -D warnings`, `cargo test`, `bin/serow check`, and `bin/serow certify`.

## 2026-05-20

- Added the first enum branching expression: `match value { Variant -> expr, Other -> expr }`.
- The checker now requires the matched expression to be an enum, requires every enum variant to be covered, rejects duplicate and unknown branch variants, and requires all branch expressions to return the same type.
- The evaluator executes only the selected branch after evaluating the matched enum expression.
- Extended `serow.ir.v0` with a `match` expression node and updated JSON output.
- Updated the Rust backend to emit exhaustive Rust `match` expressions over generated enum types.
- Added regression coverage for successful exhaustive matching, missing cases, duplicate cases, unknown variants, non-enum matched expressions, branch type mismatch, IR JSON, generated Rust source, and compiled generated Rust tests.
- Chose binary-entrypoint enum support cleanup because the Rust backend already accepted declared enum return types for `--emit-bin`, but diagnostics and docs still described only declared records.
- Added regression coverage that compiles and runs a generated binary crate whose public zero-argument `main` returns a declared enum, proving the generated entrypoint prints the derived `Debug` variant.
- Updated binary entrypoint diagnostics, agent bootstrap text/JSON, README, language notes, current state, and project rules to describe declared record/enum return support consistently.
- Chose dependency-free project manifest parser hardening as a small production-readiness cleanup because architecture policy and project version loading depend on this parser before checks run.
- Updated `serow.project` string parsing to decode JSON string escapes in keys and values, including `\uNNNN` escapes, instead of treating escaped characters as literal following bytes.
- Added regression coverage for escaped project version keys, version values, architecture module names, and dependency names.
- Chose duplicate function-parameter rejection as a small production-readiness cleanup because duplicate names were accepted at parse/patch boundaries but later evaluator and backend maps collapsed or renamed them inconsistently.
- Added a `DuplicateParameter` parse diagnostic that rejects repeated function parameter names and omits the duplicate binding from the parsed signature.
- Updated structured patch signature parsing so `patch add-function` and `patch set-signature` reject duplicate parameter names before writing.
- Added regression coverage for source parsing/checking and patch-created signatures.
- Verified with `bin/serow query intent "reject duplicate parameter names in function signatures"`, `bin/serow query symbol "parse_params"`, targeted duplicate-parameter tests, `bin/serow check`, `bin/serow certify`, `cargo test`, `cargo fmt -- --check`, and `cargo clippy --all-targets -- -D warnings`.
- Chose Python reference parser path diagnostics as a low-risk cleanup because the Rust bootstrap reports explicit missing source paths and empty source directories, while the temporary Python bootstrap silently treated them as an empty program.
- Added diagnostics-aware source discovery to `serowlang.parser`, preserving the existing `discover_sources` helper while routing `parse_files` through `SourceNotFound` and `NoSerowSources` diagnostics.
- Added Python regressions for explicit missing `.serow` files and empty source directories.
- Verified with `bin/serow query intent "report missing source paths during parsing"`, `bin/serow query symbol "SourceNotFound"`, `cargo fmt --check`, targeted Rust missing-source coverage, `python3 -m unittest discover -s tests`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bin/serow fmt --check --json`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, and `git diff --check`.
- Chose compile-rust usage-output protocol cleanup because `--json` detection for parse errors incorrectly scanned arguments after `--`, where flag-looking values are source paths.
- Updated `compile rust` usage-error JSON detection to stop at the path separator and added a CLI regression proving a separated `--json` path no longer changes a usage error from stderr text to stdout JSON.
- Chose direct effect ledger lookup as the next v1 closure task because effect declarations are public contract surface and plan/check already infer direct-call capability requirements, but agents had to read broader plan output to audit one symbol.
- Added `bin/serow query effects <symbol-or-name> [paths...] [--json]`, reporting declared effects, declared concrete capabilities, inferred concrete capabilities required by resolved direct callees, missing/unused direct-call capability deltas, the suggested canonical effect declaration, and contributing direct callees/call sites including compiler-owned intrinsics.
- Updated command discovery, README, `serow.project`, language notes, roadmap, and current-state progress notes to advertise the new query.
- Verified with `bin/serow query intent "inspect effect capabilities required by a public symbol" --json`, `bin/serow query symbol effects --json`, `cargo fmt --check`, `cargo test query -- --nocapture`, `bin/serow query effects @core.rpg.main.v1 examples/rpg.serow --json`, `bin/serow agent commands --json`, `git diff --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow plan --json`, `bin/serow certify --profile unattended --json`, and `bin/serow agent --json`.
- Chose repair-action command contract closure for `query effects` because the effect ledger query was public but the certification repair-action validator still accepted only the older query subcommands.
- Added `query effects` to the structured repair-action command validator and extended the repair-action contract regression with a synthetic effect lookup command.
- Updated `serow.project` to version `0.4.113-rust-bootstrap`.
- Verified with `bin/serow query intent "validate structured repair action command for query effects" --json`, `bin/serow query symbol "validate_repair_actions" --json`, `cargo fmt --check`, targeted repair-action contract regression, `bin/serow check --json`, `bin/serow certify --json`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow query effects @core.rpg.main.v1 examples/rpg.serow --json`, `bin/serow agent commands --json`, and `git diff --check`.
- Chose missing-section structured patch closure because Phase 2.5 still listed adding missing public sections from skeletons as open, while the implemented patch commands already covered the v1 section set.
- Added regression coverage proving `MissingRequiredSection` repair commands can create a missing `effects pure` declaration and a typed-hole `impl` section without raw source editing.
- Updated CLI/language/progress documentation to state that `patch set-effects` creates missing effect declarations as well as replacing existing declarations, marked missing-section patch coverage done enough for v1, and bumped `serow.project` to `0.4.114-rust-bootstrap`.
- Verification is recorded in the final run for this change.

## 2026-05-24

- Chose Phase 3 backend closure as the next v1 task because the implemented portable IR and Rust backend slice now has runtime contracts, generated evidence tests, metadata sidecars, artifact drift checks, and binary entrypoint support.
- Marked the first Phase 3 backend slice closed for public v1 in the roadmap and current-state notes, with unsupported future backend targets and broader language constructs explicitly scoped to v2+.
- Updated `serow.project`, README, and the compact `bin/serow agent` bootstrap text/JSON so project metadata and agent discovery now agree that public v1 backend closure is complete.
- Verified with `bin/serow query intent "close v1 release polish backend certification generated Rust artifacts"`, `bin/serow query symbol agent --json`, `cargo fmt --check`, targeted compact-agent regression, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, and `git diff --check`.
- Chose public language-reference release polish because v1 behavior was documented in `Progress/language-v0.md` but not exposed as stable user-facing documentation next to the CLI and backend references.
- Added `docs/language.md` covering the public v1 source shape, supported types and expressions, executable evidence, modules, symbol resolution, effects, built-in helpers, formatting/patching, planning/certification gates, backend boundary, and explicit v1 limits.
- Linked the language reference from the README and recorded the release-documentation closure in the roadmap and current-state notes.
- Bumped `serow.project` to `0.4.122-rust-bootstrap`.
- Verified with `bin/serow check --json`, `bin/serow certify --json`, `bin/serow fmt --check --json`, `git diff --check`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, and `python3 -m unittest discover -s tests`.
- Chose CLI documentation discovery as a release-polish closure because stable public references now live in `docs/`, but command discovery did not provide a direct docs entrypoint.
- Added `bin/serow docs [--json]`, listing the stable local language, CLI, backend, agent-instruction, and progress references in text or machine-readable form.
- Updated top-level help, compact/full agent command discovery, README, CLI docs, `serow.project`, and Progress notes to advertise the docs entrypoint.
- Bumped `serow.project` to `0.4.130-rust-bootstrap`.
- Verified with `bin/serow query intent "list public documentation references from the CLI" --json`, `bin/serow query symbol docs --json`, `cargo fmt --check`, targeted CLI command discovery tests, `bin/serow docs --json`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `bin/serow agent --json`, `bin/serow compile rust examples/math.serow --json`, `cargo test`, `cargo clippy -- -D warnings`, and `git diff --check`.
- Chose documentation reference validation as the next release-polish closure because `bin/serow docs` exposed stable local references but could not fail an automation run when one drifted or disappeared.
- Added `bin/serow docs --check [--json]`, per-reference existence fields in docs JSON output, missing-reference summaries, and non-zero check-mode status when an advertised local reference is absent.
- Updated top-level help, agent command discovery, README, CLI docs, `serow.project`, and Progress notes to advertise the validating docs entrypoint.
- Bumped `serow.project` to `0.4.131-rust-bootstrap`.
- Verified with `bin/serow query intent "release documentation discovery and command reference consistency" --json`, `bin/serow query symbol docs --json`, targeted docs/help/agent command tests, `bin/serow docs --check --json`, `bin/serow help --json`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, and `git diff --check`.
- Chose public release-gate aggregation as the next release-polish closure because the individual v1 gates existed but agents still had to assemble the Serow-owned release readiness sequence by hand.
- Added `bin/serow release-check [paths...] [--json]`, aggregating documentation reference validation, canonical format checking, standard certification, and unattended certification over the selected source set.
- Added release-check command discovery to top-level help, compact/full agent command JSON, README, CLI docs, `serow.project`, and Progress notes; the repair-action command validator now accepts the release-check command as a valid Serow protocol command.
- Corrected the current known-limits text to acknowledge implemented exhaustive nullary enum matches while leaving payload/wildcard matching in v2 scope.
- Bumped `serow.project` to `0.4.132-rust-bootstrap`.
- Verified with `bin/serow query intent "run public v1 release readiness checks" --json`, `bin/serow query symbol "release" --json`, `bin/serow query intent "fix public documentation known limits for implemented enum match expressions" --json`, `bin/serow query symbol "match" --json`, `cargo fmt --check`, targeted release-check and command-discovery regression tests, `bin/serow release-check --json`, `bin/serow help --json`, `bin/serow docs --check --json`, `cargo clippy -- -D warnings`, `python3 -m unittest discover -s tests`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `bin/serow plan --json`, `cargo test`, `bin/serow agent --json`, `bin/serow agent commands --json`, and `git diff --check`.
- Chose standard-library reference documentation as the next v1 closure task because `examples/stdlib.serow` is public, certified source-level behavior but was only summarized from the language reference and not part of the stable validated docs set.
- Added `docs/stdlib.md` covering public v1 standard-library modules, functions, record types, usage, semantic query examples, and future-scope limits.
- Added the standard-library reference to `bin/serow docs [--check] [--json]`, README references, CLI/language docs, `serow.project`, and Progress notes.
- Bumped `serow.project` to `1.0.4-rust-bootstrap`.
- Verified with `bin/serow query intent "publish public standard library reference documentation" --json`, `bin/serow query symbol "standard library" --json`, `cargo fmt --check`, `cargo test docs_command_lists_and_checks_public_references -- --nocapture`, `bin/serow docs --check --json`, `bin/serow release-check --json`, `bin/serow fmt --check --json`, `bin/serow check --json`, `bin/serow certify --json`, `bin/serow certify --profile unattended --json`, `cargo test`, `python3 -m unittest discover -s tests`, `bin/serow version --json`, `git diff --check`, and `cargo clippy -- -D warnings`.

## 2026-06-01

- Chose Rust backend CLI compatibility as a targeted production cleanup because `compile rust` only accepted two-token value flags even though equals-value forms are common in automation and avoid ambiguity around generated paths.
- Added `--out-dir=<dir>` and `--crate-name=<name>` support for `bin/serow compile rust`, preserving duplicate-flag validation, generated crate-name validation, and `--check-out-dir` artifact checking.
- Updated backend/current-state documentation and bumped `serow.project` to `1.0.29-rust-bootstrap`.
- Verified with `bin/serow query intent "compile rust option parsing accepts equals-style flags for out-dir and crate-name"`, `bin/serow query symbol "parse_compile_rust_args"`, `cargo fmt --check`, `cargo test compile_rust_accepts_equals_value_backend_flags -- --nocapture`, `bin/serow check`, `bin/serow certify`, `bin/serow docs --check --json`, `bin/serow release-check --json`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
- Chose documentation consistency cleanup because the language reference still listed list `forall` sample generation as a v1 exclusion even though the checker, CLI reference, backend docs, README, and current progress state describe bounded homogeneous `List<T>` property samples as supported.
- Updated the language known-limits text to keep richer list APIs and custom generators in future scope without incorrectly excluding supported list property samples.
- Updated current progress known limits to include bounded homogeneous list samples in the sampled-property summary.
- Verified with `bin/serow docs --check --json`, `bin/serow check`, `bin/serow certify`, `bin/serow certify --profile unattended`, `bin/serow release-check --json`, `cargo clippy --quiet --all-targets -- -D warnings`, `cargo test --quiet`, and `git diff --check`.
- Chose documentation-link hardening as a low-risk production cleanup because `bin/serow docs --check` validated local Markdown links but treated URL-encoded path characters such as `%20` literally, reporting existing files with spaces as missing.
- Decoded percent-encoded local Markdown link paths before filesystem resolution, preserved readable decoded paths in missing-link diagnostics, documented the behavior, and added a focused CLI regression.
- Verified with `bin/serow query intent "validate percent encoded local markdown links in documentation" --json`, `bin/serow query symbol "docs" --json`, `cargo fmt --check`, and `cargo test docs_check -- --nocapture`.
- Chose escaped Markdown link handling as a focused docs-checker hardening task because `bin/serow docs --check` could treat backslash-escaped link-looking text as a real local link or reference usage.
- Updated the Markdown scanner to only validate unescaped inline/reference link syntax, documented the behavior, and added a regression for escaped inline, full-reference, and collapsed-reference link text.
- Verified with `bin/serow query intent "ignore escaped markdown link syntax in documentation checks"`, `bin/serow query symbol "markdown_link_targets"`, `cargo fmt --check`, `cargo test docs_check -- --nocapture`, `bin/serow docs --check --json`, `bin/serow check`, `bin/serow certify`, `cargo clippy --all-targets -- -D warnings`, `bin/serow certify --profile unattended`, `bin/serow release-check --json`, `cargo test`, and `git diff --check`.
- Chose setext Markdown heading anchor support as a focused docs-checker hardening task because `bin/serow docs --check` recognized ATX `#` headings but could falsely report links to standard underlined Markdown headings as broken.
- Added setext heading anchor collection with duplicate-anchor suffix handling shared with ATX headings, documented the behavior, and added a regression for valid duplicated setext anchors plus a genuinely missing anchor.
- Verified with `bin/serow query intent "validate markdown heading anchors in documentation checks"`, `bin/serow query symbol "markdown_heading_anchors"`, `cargo fmt -- --check`, `cargo test docs_check -- --nocapture`, `bin/serow docs --check --json`, `bin/serow check`, `bin/serow certify`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `bin/serow certify --profile unattended`, `python3 -m unittest discover -s tests`, `git diff --check`, and `bin/serow release-check --json`.
- Chose standard-library documentation drift prevention as a low-risk production cleanup because `docs/stdlib.md` is now a validated public reference but did not have regression coverage proving it lists every checked public stdlib function.
- Added a Rust regression that parses `examples/stdlib.serow` and fails when any public function signature is absent from `docs/stdlib.md`.
- Corrected current progress notes to include Float in the source-level `core.list` helper summary.
