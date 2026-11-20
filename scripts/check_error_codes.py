#!/usr/bin/env python3
"""Enforce append-only OurDAO contract error codes."""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass

VARIANT_RE = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*,")
GAP_COMMENT_RE = re.compile(r"reserved|gap|skip|section|----|intentional|concurrent", re.I)


@dataclass(frozen=True)
class Variant:
    name: str
    value: int
    line: int
    comments: str


def parse_error_enum(path: pathlib.Path) -> list[Variant]:
    variants: list[Variant] = []
    in_enum = False
    comments: list[str] = []

    for line_no, line in enumerate(path.read_text().splitlines(), 1):
        stripped = line.strip()
        if not in_enum:
            if stripped.startswith("pub enum Error"):
                in_enum = True
            continue
        if stripped.startswith("}"):
            break
        if stripped.startswith("//"):
            comments.append(stripped)
            continue
        match = VARIANT_RE.match(line)
        if match:
            variants.append(
                Variant(match.group(1), int(match.group(2)), line_no, "\n".join(comments))
            )
            comments = []
        elif stripped and not stripped.startswith("#"):
            comments = []

    if not variants:
        raise ValueError(f"no Error variants found in {path}")
    return variants


def materialize_base(base_ref: str, error_file: str) -> pathlib.Path:
    content = subprocess.check_output(
        ["git", "show", f"{base_ref}:{error_file}"], text=True
    )
    handle = tempfile.NamedTemporaryFile("w", delete=False, suffix=".rs")
    with handle:
        handle.write(content)
    return pathlib.Path(handle.name)


def validate(base: list[Variant], current: list[Variant]) -> list[str]:
    errors: list[str] = []
    base_by_name = {v.name: v for v in base}
    current_by_name = {v.name: v for v in current}

    seen_values: dict[int, str] = {}
    for variant in current:
        previous = seen_values.get(variant.value)
        if previous is not None:
            errors.append(
                f"error code {variant.value} is reused by {previous} and {variant.name}"
            )
        seen_values[variant.value] = variant.name

    for name, old in base_by_name.items():
        new = current_by_name.get(name)
        if new is None:
            errors.append(f"existing variant {name}={old.value} was removed")
        elif new.value != old.value:
            errors.append(
                f"existing variant {name} changed from {old.value} to {new.value}"
            )

    base_max = max(v.value for v in base)
    new_variants = [v for v in current if v.name not in base_by_name]
    last_value = base_max
    for variant in new_variants:
        if variant.value <= base_max:
            errors.append(
                f"new variant {variant.name}={variant.value} must append above existing max {base_max}"
            )
            continue
        if variant.value > last_value + 1 and not GAP_COMMENT_RE.search(variant.comments):
            errors.append(
                f"new variant {variant.name}={variant.value} skips code(s) after {last_value} "
                "without a nearby explanatory comment"
            )
        last_value = max(last_value, variant.value)

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-ref", default="origin/main")
    parser.add_argument("--error-file", default="contracts/dao/src/error.rs")
    parser.add_argument("--base-file")
    args = parser.parse_args()

    current_path = pathlib.Path(args.error_file)
    cleanup: pathlib.Path | None = None
    try:
        if args.base_file:
            base_path = pathlib.Path(args.base_file)
        else:
            base_path = materialize_base(args.base_ref, args.error_file)
            cleanup = base_path

        failures = validate(parse_error_enum(base_path), parse_error_enum(current_path))
    finally:
        if cleanup:
            cleanup.unlink(missing_ok=True)

    if failures:
        for failure in failures:
            print(f"ERROR: {failure}", file=sys.stderr)
        return 1

    print("PASS: contract error codes are append-only, unique, and documented")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
