import tempfile
import unittest
from pathlib import Path

from serowlang.checker import check_program
from serowlang.ledger import ledger_symbols, query_intent, query_symbol
from serowlang.parser import parse_files


class BootstrapTests(unittest.TestCase):
    def test_sample_program_checks(self):
        program, parse_diagnostics = parse_files(["examples"])
        summary = check_program(program, parse_diagnostics)
        self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])
        self.assertEqual(summary.functions, 89)
        self.assertEqual(summary.examples, 201)
        self.assertEqual(summary.properties, 89)

    def test_explicit_missing_source_path_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "missing.serow"
            program, parse_diagnostics = parse_files([str(source)])
            self.assertEqual(program.functions, [])
            diagnostic = next(
                diagnostic
                for diagnostic in parse_diagnostics
                if diagnostic.code == "SourceNotFound"
            )
            self.assertEqual(diagnostic.target, str(source))
            self.assertIn("does not exist", diagnostic.message)

    def test_explicit_empty_source_directory_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            program, parse_diagnostics = parse_files([directory])
            self.assertEqual(program.functions, [])
            diagnostic = next(
                diagnostic
                for diagnostic in parse_diagnostics
                if diagnostic.code == "NoSerowSources"
            )
            self.assertEqual(diagnostic.target, directory)
            self.assertIn("No `.serow` source files found", diagnostic.message)

    def test_python_parser_preserves_module_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "module_deps.serow"
            source.write_text(
                """module app.main
use core.math
use core.math

type Box = { value: Int }
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            self.assertEqual(parse_diagnostics, [])
            self.assertIn("app.main", program.modules)
            dependencies = program.modules["app.main"].dependencies
            self.assertEqual(len(dependencies), 1)
            self.assertEqual(dependencies[0].module, "core.math")
            self.assertEqual(dependencies[0].source_path, str(source))
            self.assertEqual(dependencies[0].line, 2)

    def test_python_parser_preserves_explicit_empty_modules(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "empty_module.serow"
            source.write_text("module app.empty\n", encoding="utf-8")
            program, parse_diagnostics = parse_files([str(source)])
            self.assertEqual(parse_diagnostics, [])
            self.assertEqual(program.functions, [])
            self.assertIn("app.empty", program.modules)
            self.assertEqual(program.modules["app.empty"].source_path, str(source))

    def test_duplicate_function_parameters_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "duplicate_params.serow"
            source.write_text(
                """module test.params

pub fn choose(x: Int, x: Int) -> Int
  intent "Return one provided value."
  contract
    ensures result == x
  examples
    choose(1, 2) == 2
  properties
    forall x: Int:
      choose(x, x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateParameter"
                    and "`x`" in diagnostic.message
                    for diagnostic in parse_diagnostics
                ),
                [diagnostic.to_dict() for diagnostic in parse_diagnostics],
            )
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateParameter"
                    for diagnostic in summary.diagnostics
                ),
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )

    def test_malformed_type_names_are_rejected_during_parse(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "malformed_types.serow"
            source.write_text(
                """module test.types

type Box = { value: }

pub fn keep(x: ) -> Int Int
  intent "Keep a malformed shape visible to parser diagnostics."
  contract
    ensures true
  examples
    keep(1) == 1
  properties
    forall x: Int:
      keep(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            messages = [diagnostic.message for diagnostic in parse_diagnostics]
            self.assertIn("Invalid record field type ``.", messages)
            self.assertIn("Invalid parameter type ``.", messages)
            self.assertIn("Invalid return type `Int Int`.", messages)
            summary = check_program(program, parse_diagnostics)
            self.assertFalse(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])

    def test_duplicate_type_declarations_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "duplicate_types.serow"
            source.write_text(
                """module test.types

type Box = { value: Int }
type Box = { label: Text }
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "DuplicateType"
            )
            self.assertIn("Box", diagnostic.message)
            self.assertIn("first", diagnostic.data)

    def test_duplicate_record_fields_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "duplicate_fields.serow"
            source.write_text(
                """module test.types

type Box = { value: Int, value: Text }
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateRecordField"
                    and "`value`" in diagnostic.message
                    for diagnostic in summary.diagnostics
                ),
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )

    def test_unknown_record_field_type_is_warned(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "unknown_record_field_type.serow"
            source.write_text(
                """module test.types

type Box = { value: Missing }
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                summary.ok,
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )
            self.assertTrue(
                any(
                    diagnostic.code == "UnknownType"
                    and "Field `value` on type `Box` uses type `Missing`" in diagnostic.message
                    for diagnostic in summary.diagnostics
                ),
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )

    def test_duplicate_enum_variants_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "duplicate_variants.serow"
            source.write_text(
                """module test.types

type Direction = North | North
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateEnumVariant"
                    and "`North`" in diagnostic.message
                    for diagnostic in summary.diagnostics
                ),
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )

    def test_lowercase_enum_variants_are_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "lowercase_variants.serow"
            source.write_text(
                """module test.types

type Status = idle | running

pub fn initial() -> Status
  intent "Return the initial status."
  contract
    ensures result == idle
  examples
    initial() == idle
  properties
    forall status: Status:
      initial() == idle
  effects pure
  impl
    idle
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            self.assertEqual(parse_diagnostics, [])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                summary.ok,
                [diagnostic.to_dict() for diagnostic in summary.diagnostics],
            )

    def test_failed_example_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "bad.serow"
            source.write_text(
                """module test.bad

pub fn add(x: Int, y: Int) -> Int
  intent "Return a deliberately wrong sum."
  contract
    ensures result == x + y
  examples
    add(2, 3) == 5
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x - y
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            codes = [diagnostic.code for diagnostic in summary.diagnostics]
            self.assertIn("ExampleFailed", codes)

    def test_typed_hole_reports_structured_obligations(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "hole.serow"
            source.write_text(
                """module test.hole

pub fn bump(x: Int) -> Int
  intent "Return one more than x."
  version v1
  contract
    requires x >= 0
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == x + 1
  effects pure
  impl
    HOLE(Int)
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "TypedHole"
            )
            self.assertEqual(diagnostic.data.get("symbol"), "@test.hole.bump.v1")
            self.assertEqual(diagnostic.data.get("expected_type"), "Int")
            obligations = diagnostic.data.get("obligations", "")
            self.assertIn("requires 1: x >= 0", obligations)
            self.assertIn("ensures 1: result == x + 1", obligations)
            self.assertIn("example 1: bump(1) == 2", obligations)
            self.assertIn("property 1: forall x: Int: bump(x) == x + 1", obligations)
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "query",
                        "type",
                        "Int -> Int",
                        str(source),
                    ]
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_missing_required_sections_include_structured_repair_actions(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "missing_sections.serow"
            source.write_text(
                """module test.missing

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "MissingRequiredSection"
            )
            payload = diagnostic.to_dict()
            self.assertIn("repair_actions", payload)
            commands = [action["command"] for action in payload["repair_actions"]]
            self.assertIn(
                [
                    "bin/serow",
                    "patch",
                    "set-effects",
                    str(source),
                    "@test.missing.id.v1",
                    "pure",
                ],
                commands,
            )
            self.assertIn(
                [
                    "bin/serow",
                    "patch",
                    "set-impl",
                    str(source),
                    "@test.missing.id.v1",
                    "HOLE(Int)",
                ],
                commands,
            )

    def test_intent_query_finds_add(self):
        program, parse_diagnostics = parse_files(["examples"])
        self.assertFalse(parse_diagnostics)
        matches = query_intent(program, "add two integers")
        self.assertTrue(matches)
        self.assertEqual(matches[0].function.name, "add")

    def test_intent_query_uses_ranked_content_tokens(self):
        program, parse_diagnostics = parse_files(["examples"])
        self.assertFalse(parse_diagnostics)

        matches = query_intent(program, "sum integers")
        self.assertTrue(matches)
        self.assertEqual(matches[0].function.name, "add")
        self.assertIn("sum", matches[0].reasons)
        self.assertIn("int", matches[0].reasons)

        stopword_matches = query_intent(program, "rank existing public functions by intent tokens")
        self.assertFalse(stopword_matches)

    def test_symbol_queries_include_declared_types_and_variants(self):
        program, parse_diagnostics = parse_files(["examples"])
        self.assertFalse(parse_diagnostics)

        type_matches = [match.to_dict() for match in query_symbol(program, "RpgState")]
        self.assertTrue(type_matches)
        self.assertEqual(type_matches[0]["kind"], "type")
        self.assertEqual(type_matches[0]["symbol"], "@core.rpg.RpgState")
        self.assertEqual(type_matches[0]["type_kind"], "record")
        self.assertEqual(type_matches[0]["fields"][0], {"name": "room", "type": "Room"})

        variant_matches = [match.to_dict() for match in query_symbol(program, "Cave")]
        self.assertTrue(
            any(
                match["kind"] == "type"
                and match["symbol"] == "@core.rpg.Room"
                and "variant" in match["reasons"]
                for match in variant_matches
            ),
            variant_matches,
        )

        symbols = ledger_symbols(program)
        self.assertTrue(
            any(
                row["kind"] == "type"
                and row["symbol"] == "@core.rpg.Command"
                and row["type_kind"] == "enum"
                and row["variants"] == [
                    "North",
                    "South",
                    "Take",
                    "Drink",
                    "Fight",
                    "Quit",
                    "Look",
                    "Unknown",
                ]
                for row in symbols
            ),
            symbols,
        )

    def test_source_declared_symbol_version_is_part_of_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "version.serow"
            source.write_text(
                """module test.version

pub fn id(x: Int) -> Int
  intent "Return x with an explicit version."
  version v2
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            self.assertFalse(parse_diagnostics)
            self.assertEqual(program.functions[0].version, "v2")
            self.assertEqual(program.functions[0].symbol, "@test.version.id.v2")
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])

    def test_qualified_references_allow_duplicate_unqualified_names(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "qualified.serow"
            source.write_text(
                """module test.version

pub fn id(x: Int) -> Int
  intent "Return x through version one."
  version v1
  contract
    ensures result == x
  examples
    @test.version.id.v1(1) == 1
  properties
    forall x: Int:
      @test.version.id.v1(x) == x
  effects pure
  impl
    x

pub fn id(x: Int) -> Int
  intent "Return x through version two."
  version v2
  contract
    ensures result == x
  examples
    test.version.id.v2(1) == 1
  properties
    forall x: Int:
      test.version.id.v2(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])

    def test_ambiguous_unqualified_calls_are_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "ambiguous.serow"
            source.write_text(
                """module test.version

pub fn id(x: Int) -> Int
  intent "Return x through version one."
  version v1
  contract
    ensures result == x
  examples
    @test.version.id.v1(1) == 1
  properties
    forall x: Int:
      @test.version.id.v1(x) == x
  effects pure
  impl
    x

pub fn id(x: Int) -> Int
  intent "Return x through version two."
  version v2
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      @test.version.id.v2(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "AmbiguousUnqualifiedCall"
            )
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "query",
                        "symbol",
                        "id",
                        str(source),
                    ]
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_unknown_function_evaluation_errors_include_symbol_lookup_repair(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "unknown_function.serow"
            source.write_text(
                """module test.unknown

pub fn bad(x: Int) -> Int
  intent "Call a helper that does not exist."
  version v1
  contract
    ensures result == x
  examples
    bad(1) == 1
  properties
    forall x: Int:
      bad(x) == x
  effects pure
  impl
    missing_helper(x)
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.data.get("unknown_function") == "missing_helper"
            )
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "query",
                        "symbol",
                        "missing_helper",
                        str(source),
                    ]
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_duplicate_public_intent_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "duplicate_intent.serow"
            source.write_text(
                """module test.intent

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x

pub fn same_id(x: Int) -> Int
  intent "return x"
  contract
    ensures result == x
  examples
    same_id(1) == 1
  properties
    forall x: Int:
      same_id(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "PossibleDuplicate"
                    and diagnostic.data.get("shared_terms") == "return, x"
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )

    def test_parser_unescapes_quoted_metadata_strings(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "escaped_metadata.serow"
            source.write_text(
                r'''module test.metadata

pub fn id(x: Int) -> Int
  intent "Return \"quoted\" path C:\\tmp."
  version v1
  migration
    implementation-change "Changed \"quoted\" path C:\\tmp."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
''',
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            self.assertEqual([], parse_diagnostics)
            function = program.functions[0]
            self.assertEqual(function.intent, 'Return "quoted" path C:\\tmp.')
            self.assertEqual(
                function.migrations[0].note,
                'Changed "quoted" path C:\\tmp.',
            )

    def test_near_duplicate_public_intent_is_warned(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "near_duplicate_intent.serow"
            source.write_text(
                """module test.intent

pub fn add(x: Int, y: Int) -> Int
  intent "Return the arithmetic sum of x and y."
  contract
    ensures result == x + y
  examples
    add(1, 2) == 3
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x + y

pub fn sum_pair(x: Int, y: Int) -> Int
  intent "Return the sum of two integers."
  contract
    ensures result == x + y
  examples
    sum_pair(1, 2) == 3
  properties
    forall x: Int, y: Int:
      sum_pair(x, y) == sum_pair(y, x)
  effects pure
  impl
    x + y
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "NearDuplicateIntent"
                    and diagnostic.severity == "warning"
                    and diagnostic.data.get("candidate") == "@test.intent.add.v1"
                    and diagnostic.data.get("shared_terms") == "sum"
                    and diagnostic.data.get("new_only_terms") == "int, two"
                    and diagnostic.data.get("candidate_only_terms") == "arithmetic"
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )

    def test_repeated_public_evidence_is_warned(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "repeated_evidence.serow"
            source.write_text(
                """module test.evidence

pub fn id(x: Int) -> Int
  intent "Return x with repeated evidence."
  contract
    requires x == x
    requires x == x
    ensures result == x
    ensures result == x
  examples
    id(1) == 1
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])
            self.assertTrue(
                all(diagnostic.severity == "warning" for diagnostic in summary.diagnostics),
                summary.diagnostics,
            )
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateExample"
                    and diagnostic.data.get("duplicate_index") == "2"
                    and any(
                        action.command[-2:] == ["@test.evidence.id.v1", "2"]
                        and action.command[2] == "remove-example"
                        for action in diagnostic.repair_actions
                    )
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateContractClause"
                    and diagnostic.data.get("kind") == "requires"
                    and any(
                        action.command[-3:] == ["@test.evidence.id.v1", "requires", "2"]
                        and action.command[2] == "remove-contract"
                        for action in diagnostic.repair_actions
                    )
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateContractClause"
                    and diagnostic.data.get("kind") == "ensures"
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )
            self.assertTrue(
                any(
                    diagnostic.code == "DuplicateProperty"
                    and diagnostic.data.get("kind") == "property"
                    and any(
                        action.command[-2:] == ["@test.evidence.id.v1", "2"]
                        and action.command[2] == "remove-property"
                        for action in diagnostic.repair_actions
                    )
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )

    def test_repeated_public_migrations_are_warned(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "repeated_migrations.serow"
            source.write_text(
                """module test.migration

pub fn id(x: Int) -> Int
  intent "Return x with repeated migration notes."
  version v1
  migration
    implementation-change "Documented implementation rewrite."
    impact-review "Reviewed dependent coverage."
    implementation-change "Documented implementation rewrite."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "DuplicateMigration"
            )
            self.assertEqual(diagnostic.severity, "warning")
            self.assertEqual(diagnostic.data.get("kind"), "implementation-change")
            self.assertEqual(diagnostic.data.get("first_index"), "1")
            self.assertEqual(diagnostic.data.get("duplicate_index"), "2")
            self.assertTrue(
                any(
                    action.command[-3:]
                    == ["@test.migration.id.v1", "implementation-change", "2"]
                    and action.command[2] == "remove-migration"
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_executable_example_without_target_call_warns_as_shallow(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "shallow_example.serow"
            source.write_text(
                """module test.example

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    1 == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "ShallowExample"
            )
            self.assertEqual(diagnostic.data.get("example_index"), "1")
            self.assertEqual(diagnostic.data.get("example"), "1 == 1")
            self.assertTrue(
                any(
                    action.command[-2:] == ["@test.example.id.v1", "1"]
                    and action.command[2] == "remove-example"
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_sampled_property_without_target_call_warns_as_shallow(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "shallow_property.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      x == x
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "ShallowProperty"
            )
            self.assertEqual(diagnostic.data.get("property_index"), "1")
            self.assertEqual(diagnostic.data.get("property"), "x == x")
            self.assertTrue(
                any(
                    action.command[-2:] == ["@test.property.id.v1", "1"]
                    and action.command[2] == "remove-property"
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_sampled_property_without_bindings_warns_as_vacuous(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "vacuous_property.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall :
      id(1) == 1
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "VacuousProperty"
            )
            self.assertEqual(diagnostic.data.get("property_index"), "1")
            self.assertEqual(diagnostic.data.get("property"), "id(1) == 1")
            self.assertTrue(
                any(
                    action.command[-2:] == ["@test.property.id.v1", "1"]
                    and action.command[2] == "remove-property"
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_sampled_property_with_unsupported_type_has_indexed_repair_action(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "unsupported_property_type.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Blob:
      id(1) == 1
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyNotExecutable"
            )
            self.assertEqual(diagnostic.data.get("property_index"), "1")
            self.assertEqual(diagnostic.data.get("unsupported_types"), "Blob")
            self.assertEqual(
                diagnostic.data.get("unsupported_reasons"),
                "Blob: unknown type `Blob`",
            )
            self.assertEqual(diagnostic.data.get("property"), "id(1) == 1")
            self.assertTrue(
                any(
                    action.command[-2:] == ["@test.property.id.v1", "1"]
                    and action.command[2] == "remove-property"
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_sampled_record_property_reports_nested_unknown_type_reason(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "nested_unsupported_property_type.serow"
            source.write_text(
                """module test.property

type Wrapper = { payload: Blob }

pub fn one() -> Int
  version v1
  intent "Return one while a wrapper property binding exists."
  contract
    ensures result == 1
  examples
    one() == 1
  properties
    forall wrapper: Wrapper:
      one() == 1
  effects pure
  impl
    1
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyNotExecutable"
            )
            self.assertEqual(diagnostic.data.get("unsupported_types"), "Wrapper")
            self.assertEqual(
                diagnostic.data.get("unsupported_reasons"),
                "Wrapper: unknown type `Blob`",
            )

    def test_sampled_properties_support_declared_records_and_enums(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "declared_property_samples.serow"
            source.write_text(
                """module test.property

type Box = { value: Int, label: Text }
type Direction = North | South

pub fn box_value(box: Box) -> Int
  version v1
  intent "Return the value stored in the box."
  contract
    ensures result == box.value
  examples
    box_value(Box { value: 1, label: "x" }) == 1
  properties
    forall box: Box:
      box_value(box) == box.value
  effects pure
  impl
    box.value

pub fn is_north(direction: Direction) -> Bool
  version v1
  intent "Report whether a direction is north."
  contract
    ensures result == (direction == North)
  examples
    is_north(North) == true
  properties
    forall direction: Direction:
      is_north(direction) == (direction == North)
  effects pure
  impl
    direction == North
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])
            self.assertEqual(summary.properties, 2)

    def test_recursive_record_property_samples_report_cycle_reason(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "recursive_property_type.serow"
            source.write_text(
                """module test.property

type Node = { next: Node }

fn id(node: Node) -> Node
  properties
    forall node: Node:
      id(node) == node
  effects pure
  impl
    node
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyNotExecutable"
            )
            self.assertEqual(diagnostic.data.get("unsupported_types"), "Node")
            self.assertEqual(
                diagnostic.data.get("unsupported_reasons"),
                "Node: recursive record sample cycle: Node -> Node",
            )
            self.assertEqual(diagnostic.data.get("recursive_record_cycles"), "Node -> Node")

    def test_sampled_property_failure_reports_replay_data(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "property_replay.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == 2
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyFailed"
            )
            self.assertEqual(diagnostic.data.get("property_index"), "1")
            self.assertEqual(diagnostic.data.get("sample_index"), "1")
            self.assertEqual(
                diagnostic.data.get("sample_seed"),
                "@test.property.id.v1#property:1#sample:1",
            )
            self.assertEqual(diagnostic.data.get("bindings"), "x=-2")
            self.assertEqual(diagnostic.data.get("shrunk_sample_index"), "3")
            self.assertEqual(
                diagnostic.data.get("shrunk_sample_seed"),
                "@test.property.id.v1#property:1#sample:3",
            )
            self.assertEqual(diagnostic.data.get("shrunk_bindings"), "x=0")
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "replay",
                        "property",
                        "@test.property.id.v1#property:1#sample:1",
                        str(source),
                    ]
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_sampled_property_evaluation_error_reports_shrunk_data(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "property_error_shrink.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int, y: Int:
      x + y != 0 or id(10) // (x + y) == 1
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyEvaluationError"
            )
            self.assertEqual(diagnostic.data.get("sample_index"), "5")
            self.assertEqual(
                diagnostic.data.get("sample_seed"),
                "@test.property.id.v1#property:1#sample:5",
            )
            self.assertEqual(diagnostic.data.get("bindings"), "x=-2, y=2")
            self.assertEqual(diagnostic.data.get("shrunk_sample_index"), "17")
            self.assertEqual(
                diagnostic.data.get("shrunk_sample_seed"),
                "@test.property.id.v1#property:1#sample:17",
            )
            self.assertEqual(diagnostic.data.get("shrunk_bindings"), "x=0, y=0")
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "replay",
                        "property",
                        "@test.property.id.v1#property:1#sample:5",
                        str(source),
                    ]
                    for action in diagnostic.repair_actions
                ),
                diagnostic.to_dict(),
            )

    def test_expanded_int_property_samples_find_larger_counterexample(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "expanded_samples.serow"
            source.write_text(
                """module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) < 10
  effects pure
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            diagnostic = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PropertyFailed"
            )
            self.assertEqual(diagnostic.data.get("sample_index"), "7")
            self.assertEqual(
                diagnostic.data.get("sample_seed"),
                "@test.property.id.v1#property:1#sample:7",
            )
            self.assertEqual(diagnostic.data.get("bindings"), "x=10")

    def test_pure_function_cannot_call_effectful_function(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "effects.serow"
            source.write_text(
                """module test.effects

pub fn read_counter(x: Int) -> Int
  intent "Return x while modeling an effectful read."
  contract
    ensures result == x
  examples
    read_counter(1) == 1
  properties
    forall x: Int:
      read_counter(x) == x
  effects [io]
  impl
    x

pub fn bad(x: Int) -> Int
  intent "Call an effectful function from a pure function."
  contract
    ensures result == x
  examples
    bad(1) == 1
  properties
    forall x: Int:
      bad(x) == x
  effects pure
  impl
    read_counter(x)
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertIn("EffectViolation", [diagnostic.code for diagnostic in summary.diagnostics])

    def test_effectful_function_must_declare_specific_called_capabilities(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "effects.serow"
            source.write_text(
                """module test.effects

pub fn read_file(x: Int) -> Int
  intent "Return x while modeling a file read."
  contract
    ensures result == x
  examples
    read_file(1) == 1
  properties
    forall x: Int:
      read_file(x) == x
  effects [io]
  impl
    x

pub fn fetch_remote(x: Int) -> Int
  intent "Return x while modeling a network request."
  contract
    ensures result == x
  examples
    fetch_remote(1) == 1
  properties
    forall x: Int:
      fetch_remote(x) == x
  effects [network]
  impl
    x

pub fn declared_io_only(x: Int) -> Int
  intent "Call a network operation while only declaring io."
  contract
    ensures result == x
  examples
    declared_io_only(1) == 1
  properties
    forall x: Int:
      declared_io_only(x) == x
  effects [io]
  impl
    fetch_remote(read_file(x))

pub fn declared_both(x: Int) -> Int
  intent "Call io and network operations while declaring both capabilities."
  contract
    ensures result == x
  examples
    declared_both(1) == 1
  properties
    forall x: Int:
      declared_both(x) == x
  effects [io, network]
  impl
    fetch_remote(read_file(x))

pub fn declared_extra(x: Int) -> Int
  intent "Call io and network operations while also declaring disk."
  contract
    ensures result == x
  examples
    declared_extra(1) == 1
  properties
    forall x: Int:
      declared_extra(x) == x
  effects [io, network, disk]
  impl
    fetch_remote(read_file(x))
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(
                any(
                    diagnostic.code == "EffectViolation"
                    and diagnostic.data.get("function") == "@test.effects.declared_io_only.v1"
                    and diagnostic.data.get("missing_effects") == "network"
                    and any(
                        action.command
                        == [
                            "bin/serow",
                            "patch",
                            "set-effects",
                            str(source),
                            "@test.effects.declared_io_only.v1",
                            "[io, network]",
                        ]
                        for action in diagnostic.repair_actions
                    )
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )
            self.assertFalse(
                any(
                    diagnostic.data.get("function") == "@test.effects.declared_both.v1"
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )
            self.assertTrue(
                any(
                    diagnostic.code == "UnusedEffectCapability"
                    and diagnostic.severity == "warning"
                    and diagnostic.data.get("function") == "@test.effects.declared_extra.v1"
                    and diagnostic.data.get("unused_effects") == "disk"
                    and any(
                        action.command
                        == [
                            "bin/serow",
                            "patch",
                            "set-effects",
                            str(source),
                            "@test.effects.declared_extra.v1",
                            "[io, network]",
                        ]
                        for action in diagnostic.repair_actions
                    )
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )

    def test_redundant_effect_declarations_warn_with_patch_repairs(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "effects.serow"
            source.write_text(
                """module test.effects

pub fn repeated(x: Int) -> Int
  intent "Echo the integer input with repeated capability metadata."
  contract
    ensures result == x
  examples
    repeated(1) == 1
  properties
    forall x: Int:
      repeated(x) == x
  effects [io, io]
  impl
    x

pub fn mixed(x: Int) -> Int
  intent "Preserve the input number while mixing pure marker into a capability list."
  contract
    ensures result == x
  examples
    mixed(1) == 1
  properties
    forall x: Int:
      mixed(x) == x
  effects [pure, io]
  impl
    x
""",
                encoding="utf-8",
            )
            program, parse_diagnostics = parse_files([str(source)])
            summary = check_program(program, parse_diagnostics)
            self.assertTrue(summary.ok, summary.diagnostics)

            duplicate = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "DuplicateEffectCapability"
            )
            self.assertEqual(duplicate.severity, "warning")
            self.assertEqual(duplicate.data.get("duplicate_effects"), "io")
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "patch",
                        "set-effects",
                        str(source),
                        "@test.effects.repeated.v1",
                        "[io]",
                    ]
                    for action in duplicate.repair_actions
                ),
                duplicate,
            )

            mixed = next(
                diagnostic
                for diagnostic in summary.diagnostics
                if diagnostic.code == "PureEffectWithCapabilities"
            )
            self.assertEqual(mixed.data.get("suggested_effects"), "[io]")
            self.assertTrue(
                any(
                    action.command
                    == [
                        "bin/serow",
                        "patch",
                        "set-effects",
                        str(source),
                        "@test.effects.mixed.v1",
                        "[io]",
                    ]
                    for action in mixed.repair_actions
                ),
                mixed,
            )


if __name__ == "__main__":
    unittest.main()
