# Contributing to Chatnip

Thanks for helping. This page covers how to get running, what a change must pass, and how a pull request is reviewed.

## Setup

1. Install Rust (stable), Node 22, pnpm and [just](https://github.com/casey/just).
2. `just deps`
3. `CHATNIP_USE_DEBUG=true just dev` runs the app on the mock accounts, so you never need real credentials to work on Chatnip.

## Before you open a pull request

- `just check` and `just lint` are clean.
- `just test` passes.
- The end to end tests that cover your change pass: `just test-e2e <module>::`, for example `just test-e2e composer::`. They run one app instance at a time on a throwaway mock home.
- New behaviour comes with a test: a unit test next to the code, and an end to end test when it is something you click.
- Every new user facing string is in both `src/lib/i18n/en` and `src/lib/i18n/nb`.

## Conventions

- Svelte 5 runes only, TypeScript everywhere, Tailwind for styling.
- Rust functions and Tauri commands are named `get_`, `post_`, `put_` and `delete_`. A registered command is automatically a `chatnip` cli command and an MCP tool, so update `src-tauri/skill/chatnip/SKILL.md` when callers need to know about it.
- A feature is modelled once and translated to each network in its provider; a network that can't do it natively gets the local version, never a missing feature.
- No code comments; names carry the meaning.
- Test data is fictional. Never put real people, companies, addresses or tokens in fixtures, mocks or screenshots.

## Pull requests

- Open the pull request against `main`. Keep it to one change.
- CI must pass, and every pull request needs an approving review from the maintainer before it can be merged.
- Describe what the change does and how you tested it.

## Reporting bugs and ideas

Open an issue with the steps to reproduce, what you expected, and what happened. For security problems, follow [SECURITY.md](SECURITY.md) instead of opening an issue.
