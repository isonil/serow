import tempfile
import unittest
from pathlib import Path

from serowlang.checker import check_program
from serowlang.ledger import query_intent
from serowlang.parser import parse_files


class BootstrapTests(unittest.TestCase):
    def test_sample_program_checks(self):
        program, parse_diagnostics = parse_files(["examples"])
        summary = check_program(program, parse_diagnostics)
        self.assertTrue(summary.ok, [diagnostic.to_dict() for diagnostic in summary.diagnostics])
        self.assertEqual(summary.functions, 3)
        self.assertEqual(summary.examples, 7)
        self.assertEqual(summary.properties, 3)

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
            self.assertIn("AmbiguousUnqualifiedCall", [diagnostic.code for diagnostic in summary.diagnostics])

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
            self.assertIn("PossibleDuplicate", [diagnostic.code for diagnostic in summary.diagnostics])

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
                    for diagnostic in summary.diagnostics
                ),
                summary.diagnostics,
            )

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


if __name__ == "__main__":
    unittest.main()
