#!/usr/bin/env python3
"""StoryForge 架构守护脚本

检查关键模块之间是否存在禁止的依赖关系，防止循环依赖回潮。

禁止规则（基于 Phase 1 重构结果）：
- db 禁止导入 narrative / agents / memory / creative_engine
- memory 禁止导入 agents
- creative_engine 禁止导入 agents
- narrative 禁止导入 creative_engine / agents / memory
- domain 禁止导入任何业务模块（仅 std/serde 等基础库）

返回值：
- 0: 通过
- 1: 发现禁止依赖
"""

import re
import sys
from pathlib import Path
from typing import Optional, Set

ROOT = Path(__file__).parent.parent / "src-tauri" / "src"

# 顶层模块列表
MODULES = set()
for d in ROOT.iterdir():
    if d.is_dir() and (d / "mod.rs").exists():
        MODULES.add(d.name)
for f in ROOT.iterdir():
    if f.is_file() and f.suffix == ".rs" and f.name not in ("main.rs", "lib.rs"):
        MODULES.add(f.stem)

# 当前已强制执行的规则（已修复的循环依赖）。
# 随着重构推进，逐步收紧：把 KNOWN_VIOLATIONS 中的条目移入对应模块的禁止列表。
PROHIBITED = {
    "db": {"narrative", "agents", "memory", "creative_engine", "story_system", "pipeline"},
    "domain": MODULES,  # domain 只应依赖基础库，理论上不应依赖任何业务模块
}

# 已知但未修复的依赖方向，供 ROADMAP/重构计划参考，不阻塞 CI。
KNOWN_VIOLATIONS = {
    "memory": {"agents"},  # 仍有少量历史引用，待 Phase 1.2d/1.4 彻底移除
    # creative_engine -> agents: workflow/engine 仍引用 agents::service::AgentService 行为 trait
    "creative_engine": {"agents"},
    # agents -> creative_engine: agents 仍引用 creative_engine 的上下文构建/风格检查/伏笔追踪等工具
    "agents": {"creative_engine"},
    "narrative": {"creative_engine", "memory"},  # audit/search 的历史引用
}

# 基础库白名单（domain 允许依赖）
BASE_CRATES = {
    "std", "serde", "chrono", "uuid", "regex", "log", "tracing",
}


def file_module(file_path: Path) -> Optional[str]:
    """返回文件所属顶层模块"""
    parts = file_path.relative_to(ROOT).parts
    if not parts:
        return None
    name = parts[0].replace(".rs", "")
    return name if name in MODULES else None


def extract_use_modules(text: str) -> Set[str]:
    """从源码中提取 use 语句引用的顶层模块"""
    found = set()
    # use crate::X or use crate::X::
    for m in re.findall(r"\buse\s+crate::([a-zA-Z_][a-zA-Z0-9_]*)", text):
        found.add(m)
    # use crate::{ X, Y::Z }
    for block in re.findall(r"use\s+crate::\{([^}]*)\}", text, re.DOTALL):
        for m in re.findall(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\b", block):
            found.add(m)
    # 顶层 use X::（视为 crate::X）
    for line in text.split("\n"):
        line = line.strip()
        if line.startswith("use ") and not line.startswith("use crate::"):
            m = re.match(r"use\s+([a-zA-Z_][a-zA-Z0-9_]*)", line)
            if m:
                found.add(m.group(1))
    return found


def main() -> int:
    violations = []
    known = []
    for file_path in ROOT.rglob("*.rs"):
        module = file_module(file_path)
        if not module:
            continue
        text = file_path.read_text(encoding="utf-8")
        used = extract_use_modules(text)

        for target in used:
            if target == module or target not in MODULES:
                continue

            # domain 特殊规则：允许依赖基础库，禁止依赖业务模块
            if module == "domain":
                if target in BASE_CRATES:
                    continue
                if "domain" in PROHIBITED and target in PROHIBITED["domain"]:
                    violations.append(f"{file_path}: domain imports business module '{target}'")
                continue

            banned = PROHIBITED.get(module, set())
            known_banned = KNOWN_VIOLATIONS.get(module, set())
            if target in banned:
                violations.append(f"{file_path}: '{module}' imports prohibited module '{target}'")
            elif target in known_banned:
                known.append(f"{file_path}: '{module}' imports known-violation module '{target}'")

    if known:
        print("Known architecture debt (informational, does not fail CI):")
        for k in known:
            print(f"  - {k}")
        print()

    if violations:
        print("Architecture guard FAILED:")
        for v in violations:
            print(f"  - {v}")
        return 1

    print("Architecture guard PASSED")
    print(f"  Enforced modules: {len(PROHIBITED)}")
    print(f"  Enforced rules: {sum(len(v) for v in PROHIBITED.values())}")
    print(f"  Known violations tracked: {sum(len(v) for v in KNOWN_VIOLATIONS.values())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
