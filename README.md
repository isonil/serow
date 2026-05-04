# Serow

Serow is an experimental AI-first programming language.

The current implementation is a bootstrap toolchain written in dependency-free Rust. It focuses on the core language workflow rather than performance:

- spec-first public functions
- mandatory executable examples
- mandatory contracts and properties
- explicit effects
- explicit and inferred module dependencies checked against `serow.project`
- duplicate public intent detection
- structured JSON diagnostics
- a semantic ledger for agent queries

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
bin/serow patch add-use examples/math.serow app.main core.math
```

Query the project ledger:

```sh
bin/serow query intent "add two integers"
bin/serow query symbol add
```

Certify the current sample program:

```sh
bin/serow certify
```

The language and compiler are intentionally incomplete. Active state and next steps are tracked under `Progress/`.

## License

Serow is licensed under the Apache License, Version 2.0. See `LICENSE`.
