# tools/cloud-kit — the developer's Claude framework, for claude.ai cloud sessions only

Plan: `docs/plans/2026-10-26-cloud-kit.plan.md` · ruling: DEC-570 · audit: `docs/research/2026-10-25-cloud-takeover-audit.md`.

## What it is

The developer's global `~/.claude` framework (CLAUDE.md, skills, hooks, `bin/`) and the phorj/twes-in project
memories, carried **encrypted** (`kit.tar.age`, age). Cloud containers start empty, so this is how a cloud session
gets the same methodology, gates and memory as the PC.

## Why it is inert on the PC

- Nothing in phorj references this folder: no hook in `.claude/settings.json`, no skill, nothing Claude Code
  auto-loads (`tools/` is not a Claude config path).
- The trigger lives in the **cloud environment's setup script**, which only ever runs in claude.ai containers.
- Every script refuses unless it is in a cloud container. `install.sh` needs `CLAUDE_CODE_REMOTE=true` **and** the
  cloud proxy dir `/root/.ccr`, plus `PHORJ_CLOUD_KIT_KEY` to decrypt. `arm.sh` needs the proxy dir. A refusal
  writes nothing. `test-cloud-kit.sh` proves it.
- GitHub only ever sees ciphertext. The private key is never in the repo.

## One-time setup (developer)

1. On the PC: `age-keygen -o ~/.config/phorj-cloud-kit.key`. Share **only** the `age1…` public key line.
2. Cloud environment settings → environment variable `PHORJ_CLOUD_KIT_KEY` = the `AGE-SECRET-KEY-1…` line.
   Never paste it into chat.
3. Cloud environment settings → setup script:
   ```sh
   add-apt-repository -y ppa:ondrej/php && apt-get update -qq \
     && apt-get install -y php8.5-cli php8.5-bcmath php8.5-mbstring php8.5-sqlite3 age
   cargo install cargo-nextest --locked
   cd phorj && git config core.hooksPath scripts/git-hooks && bash tools/cloud-kit/arm.sh
   ```
4. Merge the session branch to `master` on the PC. New cloud sessions branch from `master`, so the kit only
   reaches a new session after the merge.

## Files

| file | role |
|---|---|
| `install.sh` | decrypt and install into `~/.claude`; `--from-plain` for raw exports; `--hook` for SessionStart |
| `arm.sh` | run by the setup script: registers one SessionStart hook in the container's `~/.claude/settings.json` |
| `pack.sh` | build `kit.tar.age` from the two exports and the public key |
| `memory-delta.sh` | encrypt the memory written in this session (new or changed files, PC slug) into `memory-deltas/` |
| `test-cloud-kit.sh` | behaviour tests on synthetic fixtures (refusals, round-trip, idempotence, unsafe archive, delta, arm) |
| `recipient.txt` | the developer's age **public** key (safe to publish) |
| `kit.tar.age`, `memory-deltas/*.age` | ciphertext only |

## What the cloud install changes, compared with the PC

- Installs the framework under the cloud slug `-home-user-<name>`.
- Prepends a cloud-adaptation block to `~/.claude/CLAUDE.md`.
- Drops the `@RTK` includes and the bundle's `loop` skill (the host provides `/loop`).
- Sets `permissions.deny` empty (DEC-570).
- Sets autonomous mode for `/home/user/phorj` through the framework's own `gates-bypass.sh --run`.
- Wires the bash firewall with one patch: `/root` is removed from `HOST_DIRS`, because HOME=/root here.
- Skips session-remember, precompact-handoff, rtk, the statusline, push-batch, the permission engine, MCP servers
  and plugins.

## Memory back to the PC

```sh
# PC (each delta, in order):
age -d -i ~/.config/phorj-cloud-kit.key tools/cloud-kit/memory-deltas/<stamp>.age | tar -x -C ~/.claude/projects
```
