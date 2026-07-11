<!--
Sync Impact Report:
- Version change: template → 1.0.0
- Modified principles: All placeholders replaced with StoryMoss-specific principles
- Added sections: Technology Stack, Build & Release Policy, UI/UX Design System
- Removed sections: None (all template placeholders filled)
- Templates requiring updates: plan-template.md, spec-template.md, tasks-template.md (core templates align)
- Follow-up TODOs: None
-->

# StoryMoss (草苔) Constitution

## Core Principles

### I. Dual-Interface Architecture (双界面架构)
StoryMoss operates on a theatrical metaphor: **Frontstage (幕前)** for immersive writing and **Backstage (幕后)** for studio management. Every feature MUST respect this duality:
- Frontstage: Distraction-free, warm-toned (#f5f4ed), Claude-reading-experience design
- Backstage: Professional dark-themed (Cinema), data-dense management interface
- Cross-interface data flows through Tauri IPC, never bypassing the bridge
- UI changes in one interface MUST NOT break the visual consistency of the other

### II. Local-First & Data Sovereignty (本地优先)
User creative content (novels, scenes, character data) is sacred. The application:
- Stores ALL user data locally in SQLite (rusqlite + r2d2 pool)
- Supports local LLM inference (Ollama-compatible) alongside cloud providers
- Vector embeddings stored in LanceDB, fully offline-capable
- Import/Export in open ZIP format — users OWN their data
- NEVER transmit creative content to external services without explicit user consent

### III. Minimal Change Principle (最小变更)
When modifying existing code:
- Make the SMALLEST possible change to achieve the goal
- Follow existing naming conventions: snake_case (Rust), camelCase (TypeScript)
- Preserve existing patterns: Result<T,E> error handling, async/await, Zustand state, TanStack Query
- If a feature can be built by extending existing code rather than replacing it, EXTEND
- Copy-paste existing working patterns before inventing new abstractions
- ZERO compiler warnings is the standard — not a goal

### IV. Build-as-Validation (构建即验证)
Every code change MUST be validated through the build pipeline BEFORE any git push:
1. `cargo check` — 0 warnings
2. `cargo test` — all tests pass (currently 20/20)
3. `npm run build` — frontend builds successfully
4. `cargo tauri build` — release bundle generated (Windows .msi + .exe)
5. Playwright screenshot verification for UI changes
- Version numbers (Cargo.toml, tauri.conf.json, package.json, package-lock.json) MUST remain synchronized
- Git tag format: `vX.Y.Z` for stable releases

### V. AI-Native Creative Workflow (AI 原生创作流)
StoryMoss is built around AI assistance, not bolted-on:
- Writer Agent: context-aware continuation with automatic follow-up triggers
- Multi-Agent System: World-building, Character, Style, Plot agents with session memory
- AI features MUST gracefully degrade when models are unavailable
- User retains FULL control: every AI suggestion is explicitly accepted (Tab) or rejected (Esc)
- Zen Mode provides pure writing environment — ALL AI UI hidden

## Technology Stack

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Desktop Runtime | Tauri | 2.4 | Cross-platform native wrapper |
| Backend | Rust | 1.94 | Core logic, IPC commands, data layer |
| Frontend Framework | React | 18 | UI components |
| Language | TypeScript | 5.8 | Type-safe frontend |
| Database | SQLite | via rusqlite | Local structured data |
| Vector DB | LanceDB | — | Semantic search, embeddings |
| Editor | TipTap / ProseMirror | — | Rich text editing |
| Styling | Tailwind CSS | — | Utility-first CSS |
| State | Zustand | — | Frontend state management |
| API Client | TanStack Query | — | Server state management |
| E2E Testing | Playwright | latest | Automated browser testing |

## Build & Release Policy

### Local Build (Windows)
```powershell
# Full release build — MUST pass before any push
.\scripts\build-local.ps1
# Outputs: src-tauri/target/release/bundle/msi/StoryMoss_0.22.2_x64_en-US.msi
#          src-tauri/target/release/bundle/nsis/StoryMoss_0.22.2_x64-setup.exe
```

### GitHub Actions (Cross-Platform)
- Push to `master` → `v0.22.2-nightly` prerelease (overwrites)
- Push tag `v*` → Stable release (independent, non-overwriting)
- Platforms: `windows-latest`, `ubuntu-latest`, `macos-latest`

## UI/UX Design System

### Frontstage (幕前)
- Background: `#f5f4ed` (warm paper)
- Text: `#2d2a26` (ink)
- Accent: `#c9a96e` (gold)
- Font: System serif stack for reading
- Paragraph style: `text-indent: 2em`, `margin-bottom: 0`
- Editor padding-bottom: `10rem` (prevents chat-toolbar overlap)

### Backstage (幕后)
- Theme: Cinema dark (`bg-cinema-900`, `text-cinema-gold`)
- Professional data-dense layout
- Sidebar + main content split

## Governance

- **Constitution Supremacy**: These principles override any ad-hoc development decisions
- **Amendments**: Require documentation in CHANGELOG.md, version bump, and explicit ratification
- **Compliance Review**: Every PR/MR must verify alignment with dual-interface architecture and local-first policy
- **Runtime Guidance**: Use AGENTS.md for day-to-day development conventions; use this Constitution for architectural decisions
- **Spec-Kit Workflow**: All new features follow `/skill:speckit-specify` → `/skill:speckit-plan` → `/skill:speckit-tasks` → `/skill:speckit-implement`

**Version**: 1.0.0 | **Ratified**: 2026-04-17 | **Last Amended**: 2026-04-17
