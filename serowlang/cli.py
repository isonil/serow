import argparse
import json
import sys
from typing import Iterable, List

from .checker import check_program
from .diagnostics import has_errors
from .ledger import ledger_symbols, query_intent, query_symbol
from .parser import parse_files


def main(argv: Iterable[str] = None) -> int:
    parser = argparse.ArgumentParser(prog="serow", description="Serow bootstrap compiler")
    subcommands = parser.add_subparsers(dest="command", required=True)

    check_parser = subcommands.add_parser("check", help="Parse and check Serow source")
    check_parser.add_argument("paths", nargs="*", help="Files or directories to check")
    check_parser.add_argument("--json", action="store_true", help="Print structured JSON diagnostics")

    certify_parser = subcommands.add_parser("certify", help="Check and require zero warnings/errors")
    certify_parser.add_argument("paths", nargs="*", help="Files or directories to certify")
    certify_parser.add_argument("--json", action="store_true", help="Print structured JSON diagnostics")

    query_parser = subcommands.add_parser("query", help="Query the semantic project ledger")
    query_subcommands = query_parser.add_subparsers(dest="query_command", required=True)

    intent_parser = query_subcommands.add_parser("intent", help="Search symbols by natural-language intent")
    intent_parser.add_argument("text")
    intent_parser.add_argument("paths", nargs="*")
    intent_parser.add_argument("--json", action="store_true")

    symbol_parser = query_subcommands.add_parser("symbol", help="Search symbols by name or stable id")
    symbol_parser.add_argument("text")
    symbol_parser.add_argument("paths", nargs="*")
    symbol_parser.add_argument("--json", action="store_true")

    symbols_parser = query_subcommands.add_parser("symbols", help="List known symbols")
    symbols_parser.add_argument("paths", nargs="*")
    symbols_parser.add_argument("--json", action="store_true")

    args = parser.parse_args(list(argv) if argv is not None else None)

    if args.command == "check":
        return _check(args.paths, json_output=args.json, certify=False)
    if args.command == "certify":
        return _check(args.paths, json_output=args.json, certify=True)
    if args.command == "query":
        return _query(args)

    parser.error("unknown command")
    return 2


def _check(paths: List[str], json_output: bool, certify: bool) -> int:
    program, parse_diagnostics = parse_files(paths)
    summary = check_program(program, parse_diagnostics)
    if json_output:
        print(json.dumps(summary.to_dict(), indent=2, sort_keys=True))
    else:
        _print_check_summary(summary, certify=certify)
    if certify:
        return 1 if summary.diagnostics else 0
    return 1 if has_errors(summary.diagnostics) else 0


def _query(args) -> int:
    paths = getattr(args, "paths", [])
    program, parse_diagnostics = parse_files(paths)
    if has_errors(parse_diagnostics):
        print(json.dumps({"ok": False, "diagnostics": [d.to_dict() for d in parse_diagnostics]}, indent=2))
        return 1

    if args.query_command == "intent":
        matches = [match.to_dict() for match in query_intent(program, args.text)]
        return _print_query(matches, args.json)
    if args.query_command == "symbol":
        matches = [match.to_dict() for match in query_symbol(program, args.text)]
        return _print_query(matches, args.json)
    if args.query_command == "symbols":
        return _print_query(ledger_symbols(program), args.json)
    return 2


def _print_check_summary(summary, certify: bool) -> None:
    mode = "certify" if certify else "check"
    status = "ok" if summary.ok and (not certify or not summary.diagnostics) else "failed"
    print(f"serow {mode}: {status}")
    print(
        "summary: "
        f"{summary.functions} functions, "
        f"{summary.examples} examples, "
        f"{summary.properties} properties, "
        f"{summary.contracts} contract checks, "
        f"{summary.holes} holes"
    )
    for diagnostic in summary.diagnostics:
        target = f" {diagnostic.target}" if diagnostic.target else ""
        print(f"{diagnostic.severity}: {diagnostic.code}:{target} {diagnostic.message}")
        if diagnostic.data:
            print(f"  data: {json.dumps(diagnostic.data, sort_keys=True)}")
        if diagnostic.repairs:
            print(f"  repairs: {', '.join(diagnostic.repairs)}")


def _print_query(rows, json_output: bool) -> int:
    if json_output:
        print(json.dumps({"ok": True, "results": rows}, indent=2, sort_keys=True))
        return 0
    if not rows:
        print("no matches")
        return 0
    for row in rows:
        score = f" score={row['score']}" if "score" in row else ""
        print(f"{row['symbol']}{score}")
        print(f"  {row['signature']}")
        if row.get("intent"):
            print(f"  intent: {row['intent']}")
        print(f"  source: {row['source']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

