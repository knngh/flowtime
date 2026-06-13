# Flowtime

> AI-powered time management & productivity desktop app. Built with Tauri v2, React 19, and Rust.

[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue?logo=tauri)](https://v2.tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178C6?logo=typescript)](https://www.typescriptlang.org/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4-06B6D4?logo=tailwindcss)](https://tailwindcss.com/)
[![License](https://img.shields.io/badge/License-MIT-green)](./LICENSE)

Flowtime intelligently manages your tasks, tracks your focus sessions, monitors app usage patterns, and learns your productivity rhythms — all from a lightweight native desktop application.

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  React 19 Frontend                    │
│  App.tsx · Components · Tailwind CSS v4               │
├──────────────────────────────────────────────────────┤
│                  Tauri v2 Bridge                       │
│  30+ invoke commands → 8 Rust backend modules         │
├──────────┬──────────┬──────────┬─────────────────────┤
│ focus.rs │ llm.rs   │ api.rs   │ tracking.rs         │
│ auto_    │ integra- │ learning │ review.rs            │
│ reply.rs │ tions.rs │ .rs      │                     │
├──────────┴──────────┴──────────┴─────────────────────┤
│              SQLite (4 migrations)                     │
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
Type tasks in plain language — Flowtime parses them into structured items using an OpenAI-compatible LLM (with heuristic fallback). AI also suggests optimal task ordering based on priority, deadlines, and dependencies.

```
Input: "明天下午3点前写完Q3规划报告，高优先级，预计2小时"
Output: ParsedTask { title: "Q3规划报告", priority: "A", duration: 120, deadline: ... }
```

### M3 — Focus Mode
Start a focus session linked to a task. Records start/end times, blocks interruptions, and tracks how many incoming messages were auto-replied during the session.

### M4 — Behavior Tracking
Every 30 seconds, Flowtime logs your active window (app name + title) via `osascript` (macOS). Aggregates into daily time distribution and productivity statistics.

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

### M7 — Weekly & Daily Reports
Dashboard views with:
- Task completion rates (with week-over-week comparison)
- High-risk task flags (approaching deadlines)
- Time distribution pie chart (coding / meeting / communication / design / entertainment / browsing)
- App categorization across **50+ apps** in 6 categories

### M8 — Behavior Learning
- **Peak hours**: Sliding window analysis (2–4 hr) over focus session data to find your most productive time blocks
- **Estimate calibration**: Weighted moving average of actual vs. estimated durations, stored per task

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri v2 |
| Frontend | React 19 + TypeScript 5.8 |
| Styling | Tailwind CSS v4 |
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
- **macOS** 12+ (window tracking uses `osascript`)

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
# Required for AI features (M2, M6)
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

> **Note**: Without `OPENAI_API_KEY`, Flowtime uses heuristic fallback algorithms for task parsing and scheduling. All other features work independently.

---

## Project Structure

```
flowtime/
├── src/                          # React frontend
│   ├── App.tsx                   # Main application component
│   ├── App.css                   # Global styles
│   ├── types.ts                  # TypeScript type definitions
│   ├── integrations.ts           # External integration API layer
│   ├── components/
│   │   └── IntegrationsPanel.tsx # External task import UI
│   ├── main.tsx                  # React entry point
│   └── vite-env.d.ts
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # App entry, migrations, command registration
│   │   ├── api.rs                # Axum HTTP API server
│   │   ├── auto_reply.rs         # M6: AI auto-reply drafts
│   │   ├── focus.rs              # M3: Focus session management
│   │   ├── integrations.rs       # M5: GitHub / Linear / Feishu
│   │   ├── learning.rs           # M8: Peak hours & estimate calibration
│   │   ├── llm.rs                # M2: NL parsing & AI scheduling
│   │   ├── review.rs             # M7: Weekly/daily reports
│   │   └── tracking.rs           # M4: Window activity logging
│   ├── Cargo.toml
│   └── tauri.conf.json           # Tauri config (window, bundle, CSP)
├── package.json
├── vite.config.ts
├── tsconfig.json
└── .gitignore
```

## Database Schema

| Table | Purpose | Key Columns |
|-------|---------|------------|
| `projects` | Project containers | id, name, color |
| `tasks` | Task items | id, title, priority, status, scheduled/actual times, project_id |
| `focus_sessions` | Focus session records | id, task_id, start/end_time, interruptions_blocked |
| `window_activity` | App usage tracking | id, date, app_name, window_title, duration_seconds |
| `pending_replies` | AI auto-reply queue | id, original_message, reply_draft, channel, status |
| `settings` | Learned parameters | key, value (peak hours, calibration ratios) |

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
```

---

## Roadmap

- [ ] Cross-platform window tracking (Windows / Linux)
- [ ] Unit & integration test suite
- [ ] User-defined app category rules
- [ ] Focus session interruption recovery
- [ ] Desktop notifications for deadline reminders
- [ ] Dark mode support

---

## License

MIT — see [LICENSE](./LICENSE) for details.
