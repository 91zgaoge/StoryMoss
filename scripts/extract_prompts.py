#!/usr/bin/env python3
"""Extract built-in prompts from src-tauri/src/prompts/registry.rs into Markdown files.

Usage:
    python scripts/extract_prompts.py

Idempotent: re-running updates existing files without changing unchanged content.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import List, Optional, Tuple

PROJECT_ROOT = Path(__file__).resolve().parent.parent
REGISTRY_PATH = PROJECT_ROOT / "src-tauri" / "src" / "prompts" / "registry.rs"
PROMPTS_ROOT = PROJECT_ROOT / "resources" / "prompts"
CARGO_TOML_PATH = PROJECT_ROOT / "src-tauri" / "Cargo.toml"

CATEGORY_DIRS = {
    "Writer": "writer",
    "Inspector": "inspector",
    "Commentator": "commentator",
    "Planner": "planner",
    "Analyzer": "analyzer",
    "Probe": "probe",
    "System": "system",
    "Memory": "memory",
    "Knowledge": "knowledge",
    "Skill": "skill",
    "Methodology": "methodology",
    "World": "world",
    "Character": "character",
    "Narrative": "narrative",
    "Pipeline": "pipeline",
    "Audit": "audit",
    "Intent": "intent",
    "Deconstruction": "deconstruction",
    "Creation": "creation",
    "Strategy": "strategy",
    "Other": "other",
}


def read_version() -> str:
    text = CARGO_TOML_PATH.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if m:
        return m.group(1)
    return "0.26.28"


def skip_string_literal(text: str, i: int) -> int:
    """Return index after a string literal starting at i, or i if not a string."""
    if i >= len(text):
        return i
    c = text[i]
    if c == '"' or c == "'":
        j = i + 1
        while j < len(text):
            if text[j] == "\\":
                j += 2
            elif text[j] == c:
                return j + 1
            else:
                j += 1
        return len(text)
    if c == "r":
        j = i + 1
        hash_count = 0
        while j < len(text) and text[j] == "#":
            hash_count += 1
            j += 1
        if j < len(text) and text[j] == '"':
            end_marker = '"' + "#" * hash_count
            k = text.find(end_marker, j + 1)
            if k >= 0:
                return k + len(end_marker)
    return i


def parse_string_literal(text: str, i: int) -> Tuple[Optional[str], int]:
    """Parse a string literal starting at i. Returns (content, end_index)."""
    if i >= len(text):
        return None, i
    c = text[i]
    if c == '"' or c == "'":
        j = i + 1
        chars = []
        while j < len(text):
            if text[j] == "\\" and j + 1 < len(text):
                esc = text[j + 1]
                if esc == "n":
                    chars.append("\n")
                elif esc == "r":
                    chars.append("\r")
                elif esc == "t":
                    chars.append("\t")
                elif esc == "\\":
                    chars.append("\\")
                elif esc == '"':
                    chars.append('"')
                elif esc == "'":
                    chars.append("'")
                else:
                    chars.append(esc)
                j += 2
            elif text[j] == c:
                return "".join(chars), j + 1
            else:
                chars.append(text[j])
                j += 1
        return None, i
    if c == "r":
        j = i + 1
        hash_count = 0
        while j < len(text) and text[j] == "#":
            hash_count += 1
            j += 1
        if j < len(text) and text[j] == '"':
            content_start = j + 1
            end_marker = '"' + "#" * hash_count
            k = text.find(end_marker, content_start)
            if k >= 0:
                return text[content_start:k], k + len(end_marker)
    return None, i


def parse_string_expr(text: str, i: int) -> Tuple[Optional[str], int]:
    """Parse a string expression like \"x\".to_string() or r#\"x\"#.to_string()."""
    while i < len(text) and text[i] in " \t\n\r":
        i += 1
    content, end = parse_string_literal(text, i)
    if content is None:
        return None, i
    j = end
    while j < len(text) and text[j] in " \t\n\r":
        j += 1
    if text.startswith(".to_string()", j):
        j += len(".to_string()")
    return content, j


def find_matching_close(text: str, start: int, open_ch: str, close_ch: str) -> int:
    """Find matching close character, skipping strings and nested pairs."""
    assert text[start] == open_ch
    depth = 1
    i = start + 1
    while i < len(text):
        if text.startswith("//", i):
            nl = text.find("\n", i)
            i = nl + 1 if nl >= 0 else len(text)
            continue
        if text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = end + 2 if end >= 0 else len(text)
            continue
        nxt = skip_string_literal(text, i)
        if nxt != i:
            i = nxt
            continue
        if text[i] == open_ch:
            depth += 1
        elif text[i] == close_ch:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def extract_vec_strings(text: str, start: int, end: int) -> List[str]:
    """Extract string literal values from a vec![...] block."""
    items: List[str] = []
    i = start
    while i < end:
        nxt = skip_string_literal(text, i)
        if nxt != i:
            s, _ = parse_string_literal(text, i)
            if s is not None:
                items.append(s)
            i = nxt
            continue
        i += 1
    return items


def parse_prompt_entry_block(block: str) -> Optional[dict]:
    """Parse a PromptEntry { ... } block."""
    fields = {}

    def find_field(field_name: str) -> int:
        pattern = re.compile(rf"\b{re.escape(field_name)}\s*:\s*")
        m = pattern.search(block)
        return m.end() if m else -1

    id_pos = find_field("id")
    if id_pos < 0:
        return None
    prompt_id, _ = parse_string_expr(block, id_pos)
    if prompt_id is None:
        return None
    fields["id"] = prompt_id

    name_pos = find_field("name")
    if name_pos < 0:
        return None
    name, _ = parse_string_expr(block, name_pos)
    fields["name"] = name or ""

    desc_pos = find_field("description")
    if desc_pos < 0:
        return None
    description, _ = parse_string_expr(block, desc_pos)
    fields["description"] = description or ""

    cat_m = re.search(r"\bcategory\s*:\s*PromptCategory::(\w+)", block)
    if not cat_m:
        return None
    category = cat_m.group(1)
    fields["category"] = category

    content_pos = find_field("default_content")
    if content_pos < 0:
        return None
    content, _ = parse_string_expr(block, content_pos)
    fields["default_content"] = content or ""

    var_m = re.search(r"\bvariables\s*:\s*vec!\[", block)
    if var_m:
        vec_start = var_m.end() - 1  # position at '['
        vec_end = find_matching_close(block, vec_start, "[", "]")
        if vec_end > vec_start:
            fields["variables"] = extract_vec_strings(block, vec_start, vec_end)
        else:
            fields["variables"] = []
    else:
        fields["variables"] = []

    return fields


def parse_macro_invocation(text: str, start: int, end: int) -> Optional[dict]:
    """Parse reg_xxx!(...) macro invocation with 5 args: id, name, desc, content, variables."""
    # Extract top-level arguments
    args: List[Tuple[int, int]] = []
    i = start
    depth = 0
    arg_start = i
    in_arg = False
    while i <= end:
        if i == end:
            # Record final argument before the closing paren.
            if depth == 1 and in_arg:
                args.append((arg_start, i))
            break
        if text[i] == '(':
            depth += 1
            if depth == 1:
                arg_start = i + 1
                in_arg = True
            i += 1
            continue
        if text[i] == ')':
            if depth == 1:
                args.append((arg_start, i))
                in_arg = False
            depth -= 1
            i += 1
            continue
        if text[i] == ',' and depth == 1:
            args.append((arg_start, i))
            arg_start = i + 1
            i += 1
            continue
        nxt = skip_string_literal(text, i)
        if nxt != i:
            i = nxt
            continue
        if text[i] in "([{":
            depth += 1
        elif text[i] in ")]}":
            depth -= 1
        i += 1

    if len(args) < 4:
        return None

    id_val, _ = parse_string_expr(text, args[0][0])
    name_val, _ = parse_string_expr(text, args[1][0])
    desc_val, _ = parse_string_expr(text, args[2][0])
    content_val, _ = parse_string_expr(text, args[3][0])

    variables: List[str] = []
    if len(args) >= 5:
        vec_start = text.find("[", args[4][0], args[4][1])
        if vec_start >= 0:
            vec_end = find_matching_close(text, vec_start, "[", "]")
            if vec_end > vec_start:
                variables = extract_vec_strings(text, vec_start, vec_end)

    if id_val is None:
        return None
    return {
        "id": id_val,
        "name": name_val or "",
        "description": desc_val or "",
        "category": "Creation",  # overridden by caller if needed
        "default_content": content_val or "",
        "variables": variables,
    }


def discover_prompts(registry_text: str) -> List[dict]:
    prompts: List[dict] = []

    # Direct m.insert(..., PromptEntry { ... }) calls
    i = 0
    while True:
        m = re.search(r"\bm\.insert\s*\(", registry_text[i:])
        if not m:
            break
        call_start = i + m.start()
        paren_start = i + m.end() - 1
        paren_end = find_matching_close(registry_text, paren_start, "(", ")")
        if paren_end < 0:
            i = call_start + 1
            continue
        call_text = registry_text[call_start:paren_end + 1]

        # Find PromptEntry { inside
        pe_match = re.search(r"PromptEntry\s*\{", call_text)
        if pe_match:
            brace_start = pe_match.end() - 1
            brace_end = find_matching_close(call_text, brace_start, "{", "}")
            if brace_end > brace_start:
                block = call_text[brace_start:brace_end + 1]
                entry = parse_prompt_entry_block(block)
                if entry:
                    prompts.append(entry)
        i = paren_end + 1

    # Macro invocations: reg_creation!(...), reg_pipeline!(...), etc.
    # Require `!(` to skip macro_rules! definitions.
    macro_re = re.compile(
        r"\b(reg_(creation|pipeline|deconstruction|creation_agent|methodology))\s*!\s*\("
    )
    i = 0
    while True:
        m = macro_re.search(registry_text[i:])
        if not m:
            break
        call_start = i + m.start()
        paren_start = registry_text.find("(", call_start)
        if paren_start < 0:
            i = call_start + 1
            continue
        paren_end = find_matching_close(registry_text, paren_start, "(", ")")
        if paren_end < 0:
            i = call_start + 1
            continue
        entry = parse_macro_invocation(registry_text, paren_start, paren_end)
        if entry:
            # Determine category from macro name
            macro_name = m.group(1)
            category_map = {
                "reg_creation": "Creation",
                "reg_pipeline": "Pipeline",
                "reg_deconstruction": "Deconstruction",
                "reg_creation_agent": "Creation",
                "reg_methodology": "Methodology",
            }
            entry["category"] = category_map.get(macro_name, "Other")
            prompts.append(entry)
        i = paren_end + 1

    # Apply m.remove("...") calls that appear after inserts.
    removed_ids: set = set()
    for m in re.finditer(r"\bm\.remove\s*\(\s*\"([^\"]+)\"\s*\)\s*;", registry_text):
        removed_ids.add(m.group(1))
    prompts = [p for p in prompts if p["id"] not in removed_ids]

    return prompts


def escape_yaml_string(value: str) -> str:
    """Escape a string for YAML double quotes if it contains special characters."""
    if not value:
        return '""'
    needs_quote = False
    if value.startswith(("{", "[", "*", "&", "!", "|", ">", "'", '"', "%", "@", "`")):
        needs_quote = True
    if ": " in value or " #" in value or value.endswith(":"):
        needs_quote = True
    if "\n" in value or "\r" in value or "\t" in value:
        needs_quote = True
    if value in ("true", "false", "null", "yes", "no", "on", "off"):
        needs_quote = True
    if not needs_quote:
        # Safe plain scalar regex (loose)
        if re.match(r"^[A-Za-z0-9_.~/-]+$", value):
            return value
        needs_quote = True
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
    return f'"{escaped}"'


def write_prompt_md(prompt: dict, version: str) -> Path:
    category = prompt["category"]
    category_dir = CATEGORY_DIRS.get(category, category.lower())
    dir_path = PROMPTS_ROOT / category_dir
    dir_path.mkdir(parents=True, exist_ok=True)

    file_path = dir_path / f"{prompt['id']}.md"
    variables = prompt.get("variables", [])
    body = prompt.get("default_content", "")

    # YAML frontmatter
    lines = [
        "---",
        f'id: {escape_yaml_string(prompt["id"])}',
        f'name: {escape_yaml_string(prompt["name"])}',
        f'description: {escape_yaml_string(prompt["description"])}',
        f'category: {escape_yaml_string(category_dir)}',
        f'version: {escape_yaml_string(version)}',
        "variables:",
    ]
    for var in variables:
        lines.append(f"  - {escape_yaml_string(var)}")
    lines.append("---")
    lines.append("")
    if body:
        lines.append(body)
    # Ensure file ends with a single newline
    content = "\n".join(lines).rstrip() + "\n"
    file_path.write_text(content, encoding="utf-8")
    return file_path


def main() -> int:
    if not REGISTRY_PATH.exists():
        print(f"ERROR: registry not found: {REGISTRY_PATH}", file=sys.stderr)
        return 1

    version = read_version()
    registry_text = REGISTRY_PATH.read_text(encoding="utf-8")
    prompts = discover_prompts(registry_text)

    if not prompts:
        print("WARNING: no prompts discovered; nothing to extract.", file=sys.stderr)
        return 0

    # Track which files are generated so we can remove stale ones
    generated: set = set()
    for prompt in prompts:
        category = prompt["category"]
        category_dir = CATEGORY_DIRS.get(category, category.lower())
        generated.add(PROMPTS_ROOT / category_dir / f"{prompt['id']}.md")

    # Remove stale .md files under PROMPTS_ROOT that are not in generated set
    for md_file in PROMPTS_ROOT.rglob("*.md"):
        if md_file not in generated:
            md_file.unlink()
            # Remove empty parent directories
            parent = md_file.parent
            while parent != PROMPTS_ROOT and parent.exists() and not any(parent.iterdir()):
                parent.rmdir()
                parent = parent.parent

    for prompt in prompts:
        write_prompt_md(prompt, version)

    print(f"Extracted {len(prompts)} prompts to {PROMPTS_ROOT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
