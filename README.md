# Serow

Serow is an experimental AI-first programming language.

The current implementation is a bootstrap toolchain written in dependency-free Rust. It focuses on the core language workflow rather than performance:

- spec-first public functions
- mandatory executable examples
- mandatory contracts and properties
- source-level public symbol versions
- qualified function references (`module.name(...)` or `@module.name.vN(...)`)
- explicit effects
- explicit and inferred module dependencies checked against `serow.project`
- duplicate public intent detection
- structured JSON diagnostics with machine-readable repair actions where available
- a semantic ledger for agent queries, including token-ranked intent search and transitive impact paths
- a first machine-readable change plan for changed symbols, impact, evidence coverage, HEAD evidence deltas, and residual risk
- unattended certification gates for explicit versions, evidence weakening against Git `HEAD`, and unchecked dependent impact

Print the current agent bootstrap contract:

```sh
bin/serow agent
bin/serow agent --json
```

Run the current checker:

```sh
bin/serow check
```

Format Serow source into the canonical textual projection:

```sh
bin/serow fmt
bin/serow fmt --check
```

Apply a structured source patch:

```sh
bin/serow patch add-contract examples/math.serow @core.math.add.v1 ensures "result == x + y"
bin/serow patch add-example examples/math.serow @core.math.add.v1 "add(2, 3) == 5"
bin/serow patch add-function examples/math.serow core.math "double(x: Int) -> Int" "Return two times x."
bin/serow patch add-property examples/math.serow @core.math.add.v1 "forall x: Int, y: Int:" "add(x, y) == add(y, x)"
bin/serow patch add-use examples/math.serow app.main core.math
bin/serow patch fill-hole examples/math.serow @core.math.double.v1 "x * 2"
bin/serow patch set-version examples/math.serow @core.math.add.v1 v1
```

Query the project ledger:

```sh
bin/serow query intent "add two integers"
bin/serow query symbol add
bin/serow query dependents @core.math.add.v1
bin/serow query impact @core.math.add.v1
```

Plan a change set:

```sh
bin/serow plan --json
bin/serow plan examples/math.serow --json
```

When a changed `.serow` file is tracked by Git, `serow plan` compares the selected public symbols against `HEAD` and reports removed or narrowed executable evidence.

Certify the current sample program:

```sh
bin/serow certify
bin/serow certify --profile unattended
```

The unattended certification profile is stricter than normal local certification. It requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default, fails when changed tracked public symbols remove or narrow executable evidence compared with Git `HEAD`, and rejects changed public symbols with transitive dependents outside the certified change set.

The language and compiler are intentionally incomplete. Active state and next steps are tracked under `Progress/`.

## License

Serow is licensed under the Apache License, Version 2.0. See `LICENSE`.
