#!/usr/bin/env python3
"""Regression tests for scripts/check_error_codes.py."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("check_error_codes.py")


def enum(*body: str) -> str:
    return "pub enum Error {\n" + "\n".join(body) + "\n}\n"


class ErrorCodeDriftTests(unittest.TestCase):
    def run_check(self, base: str, current: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            base_file = root / "base.rs"
            current_file = root / "current.rs"
            base_file.write_text(base)
            current_file.write_text(current)
            return subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--base-file",
                    str(base_file),
                    "--error-file",
                    str(current_file),
                ],
                text=True,
                capture_output=True,
            )

    def test_unchanged_and_contiguous_append_pass(self) -> None:
        base = enum("    A = 1,", "    B = 2,")
        current = enum("    A = 1,", "    B = 2,", "    C = 3,")
        self.assertEqual(self.run_check(base, current).returncode, 0)

    def test_renumbered_existing_variant_fails(self) -> None:
        base = enum("    A = 1,", "    B = 2,")
        current = enum("    A = 1,", "    B = 9,")
        self.assertNotEqual(self.run_check(base, current).returncode, 0)

    def test_reused_code_fails(self) -> None:
        base = enum("    A = 1,", "    B = 2,")
        current = enum("    A = 1,", "    B = 2,", "    C = 2,")
        self.assertNotEqual(self.run_check(base, current).returncode, 0)

    def test_unexplained_skip_fails(self) -> None:
        base = enum("    A = 1,", "    B = 2,")
        current = enum("    A = 1,", "    B = 2,", "    C = 4,")
        self.assertNotEqual(self.run_check(base, current).returncode, 0)

    def test_commented_reserved_gap_passes(self) -> None:
        base = enum("    A = 1,", "    B = 2,")
        current = enum(
            "    A = 1,",
            "    B = 2,",
            "    // Code 3 reserved for a concurrent ABI addition.",
            "    C = 4,",
        )
        self.assertEqual(self.run_check(base, current).returncode, 0)


if __name__ == "__main__":
    unittest.main()
