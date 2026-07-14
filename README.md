# Flowtime

> AI-powered time management & productivity desktop app. Built with Tauri v2, React 19, and Rust.

[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue?logo=tauri)](https://v2.tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178C6?logo=typescript)](https://www.typescriptlang.org/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4-06B6D4?logo=tailwindcss)](https://tailwindcss.com/)
[![CI](https://github.com/knngh/flowtime/actions/workflows/ci.yml/badge.svg)](https://github.com/knngh/flowtime/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-green)](./LICENSE)

Flowtime intelligently manages your tasks, tracks your focus sessions, monitors app usage patterns, and learns your productivity rhythms — all from a lightweight native desktop application.

> **Current version: v3.0** — full second-round review applied. Frontend/backend type drift fixed, desktop notifications and global shortcuts are now wired end-to-end, productivity metrics are real (no more hardcoded zeros), and the app ships dark mode, cross-platform tracking, calendar scheduling, custom app categories, and CI.

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  React 19 Frontend                    │
│  App.tsx · Components · Tailwind CSS v4 (dark mode)   │
├──────────────────────────────────────────────────────┤
│                  Tauri v2 Bridge                       │
│   30+ invoke commands → 9 Rust backend modules         │
├──────────┬──────────┬──────────┬─────────────────────┤
│ focus.rs │ llm.rs   │ api.rs   │ tracking.rs         │
│ auto_    │ integra- │ learning │ review.rs            │
│ reply.rs │ tions.rs │ .rs      │ llm_common.rs        │
├──────────┴──────────┴──────────┴─────────────────────┤
│              SQLite (6 migrations)                     │
│  projects · tasks · focus_sessions · window_activity  │
│  pending_replies · settings                           │
├──────────────────────────────────────────────────────┤
│         External Services (optional)                   │
│  OpenAI API · GitHub REST · Linear GraphQL            │
│  飞书日历 · Axum HTTP Server (localhost:random)        │
└──────────────────────────────────────────────────────┘
```

## Feature Modules

### M1 — Project & Task Management
Full CRUD for projects and tasks with priority levels (**A/B/C**), estimated durations, tags, and manual drag-to-reorder. All data persisted in local SQLite.

### M2 — Natural Language Parsing & AI Scheduling
Type tasks in plain language — Flowtime parses them into structured items using an OpenAI-compatible LLM (with heuristic fallback). AI also suggests optimal task ordering **and writes real `scheduled_start` / `scheduled_end` time slots** into your calendar, back-to-back from the start of your peak productivity window (or 09:00).

**💡 Works with Ollama**: Set `OLLAMA_API_BASE=http://localhost:11434/v1` to use local models (qwen2.5, llama3, etc.) — no cloud API key needed.

```
Input: "明天下午3点前写完Q3规划报告，高优先级，预计2小时"
Output: ParsedTask { title: "Q3规划报告", priority: "A", duration: 120, deadline: ... }
```

### M3 — Focus Mode
Start a focus session linked to a task. **Pause / Resume** at any time — interruptions are tracked and counted. When you start a session, Flowtime auto-detects if you're in your peak productivity hours and shows a subtle linkage note. Records start/end times and tracks how many incoming messages were auto-replied.

- **Peak-hours linkage**: the start-focus result returns `peak_hours_note` + `in_peak_hours`; the frontend displays the note as a transient banner (P0-1).
- **Pause/Resume UI** is fully wired to `pause_focus_session` / `resume_focus_session` (P0-2).

### M4 — Behavior Tracking
Every 30 seconds, Flowtime logs your active window (app name + title). Implementation is **cross-platform** (P3-2):
- **macOS** → `osascript`
- **Windows** → PowerShell (`Get-Process`)
- **Linux** → `xdotool`

Aggregates into daily time distribution and productivity statistics.

### M5 — External Integrations
Import tasks from three external sources with a single click:

| Source | API | Auth |
|--------|-----|------|
| GitHub Issues | REST API v3 | `GITHUB_TOKEN` |
| Linear Issues | GraphQL | `LINEAR_API_KEY` |
| 飞书 Calendar | Open API | `FEISHU_APP_ID` + `FEISHU_APP_SECRET` |

Returns `ImportResult { tasks, errors }` — partial failures don't block successful imports.

### M6 — AI Auto-Reply
When in focus mode, incoming messages trigger an LLM-generated reply draft. Manage drafts through a full lifecycle: **pending → edit → send / discard**. Falls back to bilingual preset replies when LLM is unavailable.

**Metrics are now real (P1-1):** every auto-reply while a focus session is active increments `messages_auto_replied`, so the weekly/daily report counters are no longer stuck at zero.

### 📱 Desktop Notifications
Native OS notifications actually fire (P0-4):
- **Focus session end** — confirms duration (triggered from `end_focus_session`).
- **Deadline reminder** — a background loop (`run_deadline_checker`, every 15 min) notifies you about tasks due within the next 2 hours (reads `scheduled_end` written by the AI scheduler).

> Notifications require permission; Flowtime requests it once on startup.

### ⌨️ Global Keyboard Shortcuts
Registered from the frontend via the `plugin-global-shortcut` JS API (version-stable, P0-3):
- `Cmd/Ctrl+Shift+F` — start a focus session (ignored if one is already active)
- `Cmd/Ctrl+Shift+O` — show & focus the Flowtime window

### 📦 Data Export & Import
Full JSON backup and restore of all data (projects, tasks, focus sessions, settings, window activity). Use for migration, backup, or data portability. Round-trip serialization is covered by unit tests.

### 🧠 Local LLM Support
Works with Ollama — set `OLLAMA_API_BASE=http://localhost:11434/v1` and `OLLAMA_MODEL=qwen2.5:7b`. No cloud API key required. The LLM config + request logic lives in a single shared module `llm_common.rs` (P2-1), removing the previous duplicated/dead-code paths (P2-4).

### M7 — Weekly & Daily Reports
Dashboard views with:
- Task completion rates (with week-over-week comparison)
- **High-risk task flags** based on a **real `deferred_count`** (incremented by the `defer_task` command, P1-2) — no longer hardcoded to 0.
- Time distribution by category (coding / meeting / communication / design / browsing / entertainment / other), using **user-defined category rules first, then a built-in heuristic** (P3-3).
- **Real interruption counts** (`interruption_count` from focus sessions) instead of the always-zero `interruptions_blocked`.

### M8 — Behavior Learning
- **Peak hours**: a single shared algorithm (`learning::compute_peak_ranges`, 2–4 hr window) is the source of truth for both the learning module and the focus module's peak-hours linkage (P1-3). Cached into `settings.peak_hours_data`.
- **Estimate calibration**: weighted moving average of actual vs. estimated durations, stored per task.

### 🌙 Dark Mode & Custom App Categories (P3-3)
- Toggle **dark mode** from Settings (persisted in `localStorage`, applied via Tailwind v4 class strategy).
- Define **custom app→category rules** in Settings; they are stored in `settings.app_category_rules` and take priority over the built-in heuristic in reports. Add/delete through the UI.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri v2 |
| Frontend | React 19 + TypeScript 5.8 |
| Styling | Tailwind CSS v4 (class-based dark mode) |
| Backend | Rust (edition 2021) |
| Database | SQLite via `tauri-plugin-sql` + `sqlx` 0.8 |
| HTTP Client | `reqwest` 0.12 (rustls-tls) |
| HTTP Server | `axum` 0.7 (embedded) |
| Logging | `log` 0.4 + `env_logger` 0.10 |
| Date/Time | `chrono` 0.4 |
| Serialization | `serde` 1 + `serde_json` 1 |

---

## Quick Start

### Prerequisites

- **Rust** 1.80+ ([rustup](https://rustup.rs/))
- **Node.js** 18+ + npm
- **macOS** 12+ · **Windows 10+** · **Linux** (window tracking is cross-platform; Linux needs `xdotool`)

### Setup

```bash
# Clone
git clone https://github.com/knngh/flowtime.git
cd flowtime

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Environment Variables

Set these before launching for full functionality:

```bash
# Option A: Local LLM via Ollama (recommended, no API key needed!)
export OLLAMA_API_BASE="http://localhost:11434/v1"
export OLLAMA_MODEL="qwen2.5:7b"   # or llama3, mistral, etc.

# Option B: OpenAI (if not using Ollama)
export OPENAI_API_KEY="sk-..."

# Optional: custom OpenAI-compatible endpoint
export OPENAI_API_BASE="https://api.openai.com/v1"
export OPENAI_MODEL="gpt-4o-mini"

# Optional: external integrations
export GITHUB_TOKEN="ghp_..."
export LINEAR_API_KEY="lin_api_..."
export FEISHU_APP_ID="cli_..."
export FEISHU_APP_SECRET="..."
```

> **Note**: Without any LLM configured, Flowtime uses heuristic fallback algorithms for task parsing and scheduling. All other features work independently.

---

## Project Structure

```
flowtime/
├── src/                          # React frontend
│   ├── App.tsx                   # Main application component
│   ├── types.ts                  # TypeScript type definitions (kept in sync w/ backend)
│   ├── focus.ts / review.ts / llm.ts / tracking.ts / auto_reply.ts / db.ts
│   ├── components/
│   │   ├── FocusMode.tsx         # M3: focus overlay (pause/resume)
│   │   ├── SettingsModal.tsx     # M9: dark mode + app categories
│   │   ├── IntegrationsPanel.tsx # M5: external task import UI
│   │   └── ...
│   ├── main.tsx                  # React entry point
│   └── index.css                 # Tailwind v4 + dark-mode variant
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # App entry, migrations (v1–v6), command registration, deadline checker
│   │   ├── api.rs                # Axum HTTP API server
│   │   ├── auto_reply.rs         # M6: AI auto-reply drafts
│   │   ├── focus.rs              # M3: Focus session management (+ peak-hours linkage)
│   │   ├── integrations.rs       # M5: GitHub / Linear / Feishu
│   │   ├── learning.rs           # M8: Peak hours & estimate calibration
│   │   ├── llm.rs                # M2: NL parsing & AI scheduling (writes real slots)
│   │   ├── llm_common.rs         # Shared LLM config + chat + json extraction (P2-1)
│   │   ├── review.rs             # M7: Weekly/daily reports + app categories + defer
│   │   ├── tracking.rs           # M4: Window activity logging (cross-platform)
│   │   └── data_io.rs            # JSON export/import
│   ├── Cargo.toml
│   └── tauri.conf.json           # Tauri config (window, bundle, CSP)
├── .github/workflows/ci.yml      # Rust `cargo test` + frontend `npm run build`
├── package.json
├── vite.config.ts
├── tsconfig.json
└── .gitignore
```

## Database Schema

| Table | Purpose | Key Columns |
|-------|---------|------------|
| `projects` | Project containers | id, name, color |
| `tasks` | Task items | id, title, priority, status, scheduled/actual times, project_id, **deferred_count, last_deferred_at** |
| `focus_sessions` | Focus session records | id, task_id, start/end_time, **status, interruption_count**, interruptions_blocked, messages_auto_replied |
| `window_activity` | App usage tracking | id, date, app_name, window_title, duration_seconds |
| `pending_replies` | AI auto-reply queue | id, original_message, reply_draft, channel, status |
| `settings` | Learned parameters | key, value (peak hours, calibration ratios, app category rules) |

---

## Built-in HTTP API

Flowtime runs an embedded Axum server on a random localhost port. The port is written to `~/.flowtime-api-port` for mobile or external tool access.

| Endpoint | Description |
|----------|------------|
| `GET /api/today/tasks` | Today's task list |
| `GET /api/today/summary` | Daily summary stats |
| `GET /api/focus/status` | Current focus session status |

---

## Development Commands

```bash
# Start dev server (hot reload)
npm run tauri dev

# Build for production
npm run tauri build

# Frontend only (browser)
npm run dev
npm run build

# Run Rust unit tests
cd src-tauri && cargo test
```

---

## Testing & CI

- **Rust unit tests** cover the pure/parsing logic across `focus`, `review`, `learning`, `llm`, `data_io`, and `integrations` modules. Run with `cargo test`.
- **GitHub Actions** (`.github/workflows/ci.yml`) runs `cargo test` and `npm run build` on every push/PR.

---

## Roadmap

- [x] Focus session pause/resume with interruption tracking
- [x] Desktop notifications for deadline reminders & focus end (actually firing)
- [x] Global keyboard shortcuts (Cmd/Ctrl+Shift+F / Cmd/Ctrl+Shift+O)
- [x] Real productivity metrics (interruptions, auto-replies, deferred count)
- [x] Single peak-hours algorithm shared across modules
- [x] Unit tests for core modules (focus, review, learning, llm, data_io, integrations)
- [x] Local LLM support via Ollama (shared config module)
- [x] Data export/import (JSON backup & restore)
- [x] Cross-platform window tracking (Windows / Linux)
- [x] User-defined app category rules
- [x] Dark mode support
- [x] Real calendar scheduling (writes scheduled_start/end)
- [x] CI (cargo test + npm build)
- [ ] Mobile companion via API server

---

## License

MIT — see [LICENSE](./LICENSE) for details.
