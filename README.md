# LearningComputer

![learning computer](assets/forever-amuro.jpg)

`LearningComputer` is an intentionally vibe coded Rust project.

Right now it is a simple task bot and terminal-first viewer over tasks that are actively managed by an AI agent. The immediate goal is to make the task state legible, colorful, and fast to inspect from a TUI without turning the app into a write path too early.

The long-term direction is broader:

- a CRUD app over tasks
- an agent plug-and-play ready database program
- a system that can impart compute intelligence over trends, patterns, and task history

Those later features are still TBD. For now, this repo is primarily a read-only view layer and data-loading foundation for externally managed task data.

## Current Shape

- typed snapshot model for the task YAML
- loader with timestamp and checksum-based reload gating
- unit tests around parsing, validation, and reload behavior

## Data

No task data is committed to this repository.

The live task file is expected to exist outside the repo and is ignored here. Current ignore rules explicitly cover:

- `tasks.yaml`
- `tasks.yml`
- `tasks.db`
- `data/tasks.yaml`

## Local Development

```bash
cargo test
```

Later phases will add the interactive TUI with hotkeys for task views, manual reloads, and detail panels.
