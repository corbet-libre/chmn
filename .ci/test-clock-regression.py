#!/usr/bin/env python3
"""Require named regression assertions when completion checks use a frozen clock."""
from pathlib import Path
import re
import subprocess

CONTROLS = [('src/azure.rs', 'provider_deadlines_cover_reservation_and_reply_without_discarding_private_reports', 'let completed = clock();', 'let completed = dispatched;'), ('src/identity.rs', 'registration_deadline_is_checked_inside_commit_and_after_the_wait', 'let completed = clock();', 'let completed = now;')]
originals = {Path(path): Path(path).read_bytes() for path, _, _, _ in CONTROLS}
try:
    for filename, name, before, after in CONTROLS:
        path = Path(filename)
        source = originals[path].decode()
        if source.count(before) != 1:
            raise SystemExit(f"clock control source drift: {filename}: {name}")
        path.write_text(source.replace(before, after))
        test = f"tests::clock::{name}"
        result = subprocess.run(
            ["cargo", "test", "--locked", "--lib", test, "--", "--exact", "--nocapture"],
            capture_output=True, text=True,
        )
        output = result.stdout + result.stderr
        # Compilation errors, empty selections and unrelated failures prove nothing.
        if (result.returncode != 101 or "test result: FAILED. 0 passed; 1 failed;" not in output
                or "assertion" not in output or "error[E" in output
                or not re.search(r"^test " + re.escape(test) + r" \.\.\. FAILED$", output, re.M)):
            print(output)
            raise SystemExit(f"frozen-clock control did not fail its regression assertion: {name}")
        print(f"Frozen completion clock rejected by regression assertion: {name}")
        path.write_bytes(originals[path])
finally:
    for path, original in originals.items():
        path.write_bytes(original)
