# Codebase Mapping Index

**Project:** CINEMA-AI v2-rust  
**Analysis Date:** 2026-05-11  
**Status:** Complete  
**Total Documentation:** 8 files, 2382 lines

---

## Quick Navigation

### For Understanding the Codebase
- **[STACK.md](./STACK.md)** — Technology stack, versions, and dependencies
- **[STRUCTURE.md](./STRUCTURE.md)** — Directory layout and module organization
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** — System design, data flow, and key components

### For Development
- **[CONVENTIONS.md](./CONVENTIONS.md)** — Code style, naming, and patterns
- **[INTEGRATIONS.md](./INTEGRATIONS.md)** — External services and API integrations
- **[TESTING.md](./TESTING.md)** — Test framework, coverage, and strategies

### For Issues & Concerns
- **[CONCERNS.md](./CONCERNS.md)** — Detailed analysis of bugs, tech debt, and fragile areas
- **[ANALYSIS_SUMMARY.md](./ANALYSIS_SUMMARY.md)** — Executive summary with fix priorities

---

## Document Overview

### STACK.md (4.9 KB)
**Purpose:** Technology inventory and dependency management

**Contains:**
- Frontend stack (React 18, TypeScript, Vite, TanStack Query)
- Backend stack (Rust, Tauri v2.4, Tokio async runtime)
- Database (SQLite with r2d2 connection pooling)
- Key dependencies and versions
- Build tools and configuration

**Use when:** You need to understand what technologies are in use, check versions, or plan dependency updates.

---

### STRUCTURE.md (11 KB)
**Purpose:** Codebase organization and module layout

**Contains:**
- Directory tree with descriptions
- Backend module organization (37 modules)
- Frontend component hierarchy
- Key files and their purposes
- Module dependencies

**Use when:** You're navigating the codebase, looking for where to add new features, or understanding how modules relate.

---

### ARCHITECTURE.md (6.5 KB)
**Purpose:** System design and high-level data flow

**Contains:**
- Multi-window architecture (frontstage/backstage)
- State synchronization mechanism
- IPC command flow
- Database schema overview
- Key design patterns

**Use when:** You need to understand how the system works end-to-end, how windows communicate, or how data flows through the system.

---

### CONVENTIONS.md (11 KB)
**Purpose:** Code style, naming patterns, and best practices

**Contains:**
- Rust conventions (naming, error handling, async patterns)
- TypeScript/React conventions (component structure, hooks, naming)
- Database conventions (table naming, foreign keys)
- IPC command naming (snake_case)
- Comment and documentation style

**Use when:** You're writing new code and want to match the project's style, or reviewing code for consistency.

---

### INTEGRATIONS.md (6.3 KB)
**Purpose:** External services and API integrations

**Contains:**
- LLM providers (OpenAI, Ollama, Claude)
- Vector databases (Qdrant, Milvus)
- Embedding providers
- MCP (Model Context Protocol) servers
- Configuration and authentication

**Use when:** You're adding new integrations, configuring external services, or understanding how the system connects to external APIs.

---

### TESTING.md (12 KB)
**Purpose:** Test framework, coverage, and testing strategies

**Contains:**
- Backend test framework (Rust #[test], property-based tests)
- Frontend test framework (Vitest, React Testing Library)
- Test organization and naming
- Coverage gaps and priorities
- Integration test strategies
- Bug condition testing methodology

**Use when:** You're writing tests, understanding test coverage, or planning test improvements.

---

### CONCERNS.md (22 KB)
**Purpose:** Detailed analysis of bugs, technical debt, and fragile areas

**Contains:**
- **Tech Debt:** 7 issues (foreign keys, cascade deletes, missing sync events, etc.)
- **Known Bugs:** 1 issue (image LLM not implemented)
- **Performance Bottlenecks:** 3 issues (clones, unwraps, large components)
- **Fragile Areas:** 3 issues (state sync, schema evolution, cooldown state)
- **Test Coverage Gaps:** 2 issues (property-based tests, frontend tests)
- **Scaling Limits:** 2 issues (connection pool, in-memory state)
- **Dependencies at Risk:** 2 issues (reqwest pinning, Tauri compatibility)
- **Missing Features:** 2 issues (image generation, plan templates)

**Each issue includes:**
- Root cause analysis
- Impact assessment
- Affected files
- Recommended fix approach
- Effort estimate

**Use when:** You're investigating a bug, planning refactoring, or assessing technical debt.

---

### ANALYSIS_SUMMARY.md (11 KB)
**Purpose:** Executive summary with prioritized fix recommendations

**Contains:**
- Overview of findings (18 concerns across 3 categories)
- Critical issues (P0) — must fix
- Functional gaps (P1) — should fix
- Code quality issues (P2) — nice to fix
- Recommended fix order (4 phases)
- Risk assessment
- Testing strategy
- Monitoring recommendations
- Effort estimates and timeline

**Use when:** You're planning work, prioritizing fixes, or reporting to stakeholders.

---

## Key Findings Summary

### Critical Issues (P0)
1. **Foreign key constraints not enforced** — Data integrity at risk
2. **Incomplete cascade deletes** — Orphaned records in 20+ tables
3. **Missing character relationship IPC** — Users can't modify relationships

### Functional Gaps (P1)
4. Knowledge graph updates not synchronized
5. Ingestion pipeline events not emitted
6. Plan template library not persisted
7. Capability evolution only at startup
8. Payoff ledger cache not invalidated
9. Workflow instances not resumed after restart

### Code Quality (P2)
10. Excessive clone operations (1441+ instances)
11. Unwrap calls in lock operations (103 instances)
12. Large component files (1000+ lines)
13. Fragile state synchronization
14. Database schema evolution not versioned
15. Ingest pipeline cooldown not persisted
16. Property-based test coverage gaps
17. Frontend sync store tests incomplete
18. Image LLM profile type not implemented

---

## Recommended Reading Order

### For New Team Members
1. STACK.md — Understand the tech stack
2. STRUCTURE.md — Learn the codebase layout
3. ARCHITECTURE.md — Understand how it all fits together
4. CONVENTIONS.md — Learn the coding style

### For Bug Fixes
1. CONCERNS.md — Find the specific issue
2. ARCHITECTURE.md — Understand the affected system
3. TESTING.md — Plan your test strategy
4. CONVENTIONS.md — Match the code style

### For Feature Development
1. STRUCTURE.md — Find where to add code
2. ARCHITECTURE.md — Understand the data flow
3. CONVENTIONS.md — Match the code style
4. INTEGRATIONS.md — Check for external dependencies

### For Planning & Prioritization
1. ANALYSIS_SUMMARY.md — Get the executive view
2. CONCERNS.md — Dive into details
3. TESTING.md — Understand validation needs

---

## Statistics

| Metric | Value |
|--------|-------|
| Total Documentation | 2,382 lines |
| Concerns Identified | 18 |
| Critical Issues (P0) | 3 |
| Functional Gaps (P1) | 9 |
| Code Quality Issues (P2) | 6 |
| Backend Modules | 37 |
| Frontend Components | 50+ |
| Database Tables | 40+ |
| IPC Commands | 157+ |
| Test Files | 8+ |
| Estimated Fix Effort | 80-120 hours |
| Critical Path | 10-14 hours |

---

## How to Use This Documentation

### Scenario 1: "I need to fix a bug"
1. Read ANALYSIS_SUMMARY.md to find the issue
2. Read the detailed description in CONCERNS.md
3. Check ARCHITECTURE.md to understand the affected system
4. Review CONVENTIONS.md to match code style
5. Check TESTING.md for test strategy

### Scenario 2: "I need to add a new feature"
1. Read STRUCTURE.md to find where to add code
2. Read ARCHITECTURE.md to understand data flow
3. Check INTEGRATIONS.md for external dependencies
4. Review CONVENTIONS.md to match code style
5. Check TESTING.md for test requirements

### Scenario 3: "I need to understand the codebase"
1. Start with STACK.md (what technologies)
2. Read STRUCTURE.md (where things are)
3. Read ARCHITECTURE.md (how things work)
4. Skim CONVENTIONS.md (how to write code)
5. Reference CONCERNS.md for known issues

### Scenario 4: "I need to report on technical debt"
1. Read ANALYSIS_SUMMARY.md for overview
2. Reference CONCERNS.md for details
3. Use statistics and effort estimates for planning
4. Share ANALYSIS_SUMMARY.md with stakeholders

---

## Maintenance

### When to Update This Documentation

- **After major refactoring:** Update STRUCTURE.md and ARCHITECTURE.md
- **After adding new dependencies:** Update STACK.md
- **After fixing a concern:** Update CONCERNS.md and ANALYSIS_SUMMARY.md
- **After adding new integrations:** Update INTEGRATIONS.md
- **After changing code style:** Update CONVENTIONS.md
- **After improving tests:** Update TESTING.md

### Version Control

These documents are stored in `.planning/codebase/` and should be committed to git whenever the codebase changes significantly. They serve as a living reference for the project.

---

## Related Documentation

- **Design Specs:** `.kiro/specs/design-implementation-alignment-v5.7/`
  - `design.md` — System design requirements
  - `bugfix.md` — Bug conditions and fixes
  - `tasks.md` — Implementation tasks
  - `test-counterexamples.md` — Test cases

- **Project Root:** 
  - `ARCHITECTURE.md` — High-level architecture (if exists)
  - `ROADMAP.md` — Feature roadmap
  - `FEATURES.md` — Feature documentation
  - `PROJECT_STATUS.md` — Current status

---

## Contact & Questions

For questions about this documentation or the codebase analysis:
- Review the relevant document section
- Check CONCERNS.md for known issues
- Refer to ARCHITECTURE.md for system design questions
- Check CONVENTIONS.md for code style questions

---

*Index created: 2026-05-11*  
*Last updated: 2026-05-11*  
*Maintained by: Kiro Codebase Mapper*
