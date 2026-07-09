#!/usr/bin/env python3
"""
Phase 1.3: IPC Command Registry Consistency Check

Verifies that all frontend invoke() / loggedInvoke() command names exist in
the backend tauri::generate_handler![] registry.

Usage:
    python scripts/verify-ipc-manifest.py

Exit codes:
    0 - Consistent (or only WARN-level differences)
    1 - ERROR: Frontend calls a command not registered in backend
"""

import os
import re
import sys
from pathlib import Path
from typing import Set, Tuple

# Project root (script is in scripts/)
ROOT = Path(__file__).parent.parent

BACKEND_LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
BACKEND_HANDLERS_RS = ROOT / "src-tauri" / "src" / "handlers.rs"
FRONTEND_SRC_DIR = ROOT / "src-frontend" / "src"


def extract_backend_commands(content: str) -> Set[str]:
    """
    Extract all command names from generate_handler![] macro.
    Supports:
        - Simple names: health_check
        - Module paths: window::show_frontstage
        - Comment lines are ignored
        - Multi-line macros with nested [] (e.g. array literals)
        - lib.rs may use include!("handlers.rs") — fall through to that file
    Returns the actual command name string (last segment of module path).
    """
    commands = set()
    lines = content.splitlines()

    # 定位 generate_handler![ 的起始行
    start_idx = None
    for i, line in enumerate(lines):
        if "generate_handler![" in line:
            start_idx = i
            break

    if start_idx is None:
        # v0.26.38: handlers live in handlers.rs via include!("handlers.rs")
        if BACKEND_HANDLERS_RS.exists():
            return extract_backend_commands(BACKEND_HANDLERS_RS.read_text(encoding="utf-8"))
        print("WARN: generate_handler![] macro not found in lib.rs or handlers.rs")
        return commands

    # Collect from start until matching ] is found
    # Use bracket depth counting
    bracket_depth = 0
    block_lines = []
    for i in range(start_idx, len(lines)):
        line = lines[i]
        for ch in line:
            if ch == "[":
                bracket_depth += 1
            elif ch == "]":
                bracket_depth -= 1
                if bracket_depth == 0:
                    block_lines.append(line)
                    break
        else:
            block_lines.append(line)
            continue
        break

    # Strip text before generate_handler![ on first line
    first = block_lines[0]
    if "generate_handler![" in first:
        first = first.split("generate_handler![", 1)[1]
    block_lines[0] = first

    # Strip text after closing ] on last line
    last = block_lines[-1]
    last_closing = last.rfind("]")
    if last_closing != -1:
        block_lines[-1] = last[:last_closing]

    for line in block_lines:
        line = line.split("//")[0].strip()
        if not line:
            continue

        # A line may contain multiple commands separated by commas
        for item in line.split(","):
            item = item.strip()
            if not item:
                continue

            # Extract command name: window::show_frontstage -> show_frontstage
            if "::" in item:
                name = item.split("::")[-1].strip()
            else:
                name = item.strip()

            if name and name not in ("",):
                commands.add(name)

    return commands


def extract_frontend_commands(directory: Path) -> Tuple[Set[str], Set[Tuple[str, str]]]:
    """
    Walk frontend src directory and extract all invoke() / loggedInvoke() call command names.

    Returns:
        - Set of command names
        - Set of (file_path, command_name) for locating errors
    """
    commands = set()
    locations = set()

    # Match invoke('command_name'), invoke<Type>('command_name'),
    # loggedInvoke('command_name') or loggedInvoke<Type>('command_name')
    pattern = re.compile(
        r"(?:invoke|loggedInvoke)\s*(?:<[^>]+>)?\s*\(\s*['\"]([a-zA-Z_][a-zA-Z0-9_]*)['\"]"
    )

    for ext in ("*.ts", "*.tsx"):
        for file_path in directory.rglob(ext):
            try:
                content = file_path.read_text(encoding="utf-8")
            except Exception as e:
                print(f"WARN: Cannot read {file_path}: {e}")
                continue

            for match in pattern.finditer(content):
                cmd = match.group(1)
                commands.add(cmd)
                rel_path = file_path.relative_to(ROOT)
                locations.add((str(rel_path), cmd))

    return commands, locations


# Known frontend stubs that call commands not yet registered (pre-existing debt).
# Do not grow this set without a tracked ROADMAP item.
FRONTEND_ORPHAN_ALLOWLIST = {
    "auto_write_cancel",
    "auto_revise_cancel",
    "get_canonical_state",
}


def main() -> int:
    if not BACKEND_LIB_RS.exists():
        print(f"ERROR: Backend file not found: {BACKEND_LIB_RS}")
        return 1

    if not FRONTEND_SRC_DIR.exists():
        print(f"ERROR: Frontend directory not found: {FRONTEND_SRC_DIR}")
        return 1

    # 1. Extract backend commands
    backend_content = BACKEND_LIB_RS.read_text(encoding="utf-8")
    backend_cmds = extract_backend_commands(backend_content)
    print(f"INFO: Backend registered commands: {len(backend_cmds)}")

    # 2. Extract frontend commands
    frontend_cmds, frontend_locations = extract_frontend_commands(FRONTEND_SRC_DIR)
    print(f"INFO: Frontend invoke calls: {len(frontend_cmds)}")

    # 3. Check differences
    frontend_only = frontend_cmds - backend_cmds
    allowlisted = frontend_only & FRONTEND_ORPHAN_ALLOWLIST
    frontend_only = frontend_only - FRONTEND_ORPHAN_ALLOWLIST
    backend_only = backend_cmds - frontend_cmds

    exit_code = 0

    if allowlisted:
        print(f"\n{'=' * 60}")
        print(f"WARN: {len(allowlisted)} frontend orphan calls allowlisted (known debt):")
        print(f"{'=' * 60}")
        for cmd in sorted(allowlisted):
            print(f"  - {cmd}")

    # ERROR: Frontend calls a command not registered in backend (runtime failure)
    if frontend_only:
        exit_code = 1
        print(f"\n{'=' * 60}")
        print(f"ERROR: {len(frontend_only)} frontend calls not registered in backend:")
        print(f"{'=' * 60}")
        for cmd in sorted(frontend_only):
            locations = [loc for loc, c in frontend_locations if c == cmd]
            for loc in locations:
                print(f"  - '{cmd}' called from: {loc}")

    # WARN: Backend commands not called by frontend (may be internal)
    if backend_only:
        print(f"\n{'=' * 60}")
        print(f"WARN: {len(backend_only)} backend commands not called by frontend:")
        print(f"(These may be internal commands or new commands not yet wired to frontend)")
        print(f"{'=' * 60}")
        for cmd in sorted(backend_only):
            print(f"  - {cmd}")

    # Success
    if exit_code == 0 and not backend_only:
        print("\n[OK] IPC registry fully consistent.")
    elif exit_code == 0 and backend_only:
        print("\n[OK] All frontend calls are registered. (Backend-only commands exist, which is normal)")

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
