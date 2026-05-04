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


if __name__ == "__main__":
    unittest.main()
