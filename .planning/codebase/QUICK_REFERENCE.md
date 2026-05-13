# Quick Reference Card

**CINEMA-AI v2-rust Codebase Analysis**  
**Generated:** 2026-05-11 | **Status:** Complete

---

## 📍 Start Here

| Need | Read | Time |
|------|------|------|
| Overview | COMPLETION_REPORT.md | 5 min |
| Navigation | INDEX.md | 3 min |
| Executive Summary | ANALYSIS_SUMMARY.md | 10 min |
| Detailed Issues | CONCERNS.md | 20 min |

---

## 🎯 Critical Issues (P0) — Fix Now

| Issue | Impact | Fix Time | File |
|-------|--------|----------|------|
| Foreign keys not enforced | Data corruption | 2-3 hrs | src/db/connection.rs |
| Cascade deletes incomplete | Orphaned records | 3-4 hrs | src/db/schema.rs |
| Missing character relationship IPC | Users can't modify | 2-3 hrs | src/ipc/commands.rs |

**Total P0 effort:** 10-14 hours

---

## 📋 Functional Gaps (P1) — Fix Soon

| Issue | Impact | Fix Time |
|-------|--------|----------|
| Knowledge graph sync missing | Stale frontend data | 3-4 hrs |
| Ingestion events not emitted | Automation broken | 3-4 hrs |
| Plan templates not persisted | Lost on restart | 2-3 hrs |
| Capability evolution at startup only | Incomplete automation | 3-4 hrs |
| Payoff ledger cache not invalidated | Stale calculations | 2-3 hrs |
| Workflow instances not resumed | Lost progress | 3-4 hrs |
| State sync fragile | Easy to miss events | 4-5 hrs |
| Property-based tests incomplete | Coverage gaps | 2-3 hrs |
| Frontend sync tests incomplete | Untested code | 2-3 hrs |

**Total P1 effort:** 25-35 hours

---

## 🔧 Code Quality (P2) — Fix Gradually

| Issue | Impact | Fix Time |
|-------|--------|----------|
| 1,441+ clone operations | Memory pressure | 15-20 hrs |
| 103 unwrap() on locks | Crash risk | 10-15 hrs |
| Large components (1000+ lines) | Hard to maintain | 15-20 hrs |
| Database migrations not versioned | Schema evolution risky | 5-8 hrs |
| Ingest cooldown not persisted | Lost state | 2-3 hrs |
| Image LLM not implemented | Feature incomplete | 5-8 hrs |

**Total P2 effort:** 45-70 hours

---

## 📊 By The Numbers

```
Total Concerns:        18
Critical (P0):         3
Functional (P1):       9
Code Quality (P2):     6

Total Effort:          80-120 hours
Critical Path:         10-14 hours
Full Resolution:       4-6 weeks

Documentation:         10 files, 124 KB
Lines Analyzed:        2,700+
Files Reviewed:        40+
Patterns Checked:      1000+
```

---

## 🗂️ Documentation Map

```
.planning/codebase/
├── COMPLETION_REPORT.md    ← Start here for overview
├── INDEX.md                ← Navigation guide
├── ANALYSIS_SUMMARY.md     ← Executive summary
├── CONCERNS.md             ← Detailed issues
├── ARCHITECTURE.md         ← System design
├── STRUCTURE.md            ← Code organization
├── CONVENTIONS.md          ← Code style
├── TESTING.md              ← Test framework
├── INTEGRATIONS.md         ← External services
├── STACK.md                ← Tech inventory
└── QUICK_REFERENCE.md      ← This file
```

---

## 🚀 Recommended Action Plan

### Week 1: Data Integrity (P0)
- [ ] Enable foreign key constraints
- [ ] Fix cascade deletes
- [ ] Add character relationship IPC
- [ ] Write integration tests
- **Effort:** 10-14 hours

### Week 2-3: Sync Events (P1)
- [ ] Emit knowledge graph sync events
- [ ] Emit ingestion completion events
- [ ] Persist plan templates
- [ ] Schedule capability evolution
- [ ] Invalidate payoff cache
- [ ] Resume workflow instances
- **Effort:** 25-35 hours

### Week 4+: Code Quality (P2)
- [ ] Optimize clone operations
- [ ] Remove unwrap() calls
- [ ] Refactor large components
- [ ] Version database migrations
- [ ] Implement image LLM
- **Effort:** 45-70 hours

---

## 🔍 Quick Lookup

### "I need to fix a bug"
1. Find issue in CONCERNS.md
2. Check ARCHITECTURE.md for context
3. Review CONVENTIONS.md for style
4. Check TESTING.md for test strategy

### "I need to add a feature"
1. Check STRUCTURE.md for where to add code
2. Review ARCHITECTURE.md for data flow
3. Check INTEGRATIONS.md for dependencies
4. Follow CONVENTIONS.md for style

### "I need to understand the system"
1. Read STACK.md (technologies)
2. Read STRUCTURE.md (organization)
3. Read ARCHITECTURE.md (design)
4. Skim CONVENTIONS.md (style)

### "I need to report on tech debt"
1. Share ANALYSIS_SUMMARY.md
2. Reference CONCERNS.md for details
3. Use effort estimates for planning
4. Share COMPLETION_REPORT.md with stakeholders

---

## ⚠️ High-Risk Areas

| Area | Risk | Mitigation |
|------|------|-----------|
| State synchronization | Easy to miss sync events | Add sync event checklist |
| Database schema | Orphaned records possible | Enable foreign keys |
| Large components | Hard to test/maintain | Refactor to <500 lines |
| Lock unwraps | Crash on panic | Replace with error handling |
| Clone operations | Memory pressure | Profile and optimize |

---

## ✅ Success Metrics

### Phase 1 (Data Integrity)
- ✅ Foreign key constraints enforced
- ✅ No orphaned records after deletions
- ✅ Character relationships modifiable from UI

### Phase 2 (Sync Events)
- ✅ All mutations emit sync events
- ✅ Frontend caches invalidate correctly
- ✅ No stale data visible to users

### Phase 3 (Automation Loops)
- ✅ Plan templates persist across sessions
- ✅ Capabilities evolve automatically
- ✅ Workflows resume after restart

### Phase 4 (Code Quality)
- ✅ No unwrap() calls on locks
- ✅ Components <500 lines
- ✅ Test coverage >80%

---

## 📞 Key Contacts

For questions about:
- **Architecture:** See ARCHITECTURE.md
- **Code style:** See CONVENTIONS.md
- **Testing:** See TESTING.md
- **Specific issues:** See CONCERNS.md
- **Tech stack:** See STACK.md
- **Organization:** See STRUCTURE.md

---

## 🔗 Related Resources

- Design specs: `.kiro/specs/design-implementation-alignment-v5.7/`
- Project root: `ARCHITECTURE.md`, `ROADMAP.md`
- Issue tracker: GitHub Issues
- CI/CD: GitHub Actions

---

## 📈 Progress Tracking

Use this to track fixes:

```
P0 Issues:
- [ ] Foreign keys (2-3 hrs)
- [ ] Cascade deletes (3-4 hrs)
- [ ] Character relationship IPC (2-3 hrs)

P1 Issues:
- [ ] Knowledge graph sync (3-4 hrs)
- [ ] Ingestion events (3-4 hrs)
- [ ] Plan templates (2-3 hrs)
- [ ] Capability evolution (3-4 hrs)
- [ ] Payoff cache (2-3 hrs)
- [ ] Workflow resume (3-4 hrs)
- [ ] State sync (4-5 hrs)
- [ ] Property tests (2-3 hrs)
- [ ] Frontend tests (2-3 hrs)

P2 Issues:
- [ ] Clone optimization (15-20 hrs)
- [ ] Unwrap removal (10-15 hrs)
- [ ] Component refactor (15-20 hrs)
- [ ] Migration versioning (5-8 hrs)
- [ ] Cooldown persistence (2-3 hrs)
- [ ] Image LLM (5-8 hrs)
```

---

*Quick Reference Card*  
*CINEMA-AI v2-rust Codebase Analysis*  
*Generated: 2026-05-11*  
*Status: Ready for Action*
