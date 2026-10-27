# Chatnip on the command line

Chatnip is one program. Started with no arguments, it opens the app. Started with arguments, it is a command line tool that can do everything the app can.

```bash
chatnip conversations get --limit 10
chatnip person get --person-id 3f2c…
chatnip search get --input '{"query":"from:anna in:design has:file"}'
chatnip event create --file event.json
chatnip draft update --set input.conversation_id=ID --set input.markdown="Half a thought"
```

## Putting it on your PATH

Installed with Homebrew, `chatnip` and `cnip` are already on your PATH. Otherwise press **Add to PATH** under Settings, Intelligence, or run:

```bash
/Applications/Chatnip.app/Contents/MacOS/chatnip path install
```

This links the binary into `~/.local/bin`. Make sure that folder is on your PATH.

## How commands are named

The grammar is `chatnip <resource> <verb>`. The verbs are `get` (reads), `create` (adds), `update` (changes) and `delete` (removes). In the engine they are named `get_`, `post_`, `put_` and `delete_`:

| Engine name | Command |
|---|---|
| `get_conversations` | `chatnip conversations get` |
| `post_message` | `chatnip message create` |
| `put_conversation_read` | `chatnip conversation-read update` |
| `delete_person_identity` | `chatnip person-identity delete` |
| `delete_lock` | `chatnip lock delete` |

**Arguments**
- Flags are the parameter names in kebab case, such as `--person-id`.
- Many commands take one `--input` object with snake case fields.
- `--set key=value` sets one argument; repeat it, and use dots to nest (`--set input.content=Hi`).
- `--data '{…}'` or `--data @file.json` passes every argument at once; `--file path` reads them from a file, `-` from stdin.

**Output**
- A table on a terminal, JSON in a pipe. `--json` forces JSON, `--raw` prints it compact, `--plain` prints tab separated lines.
- `--limit`, `--cursor`, `--fields a,b.c`, `--sort field[:desc]`, `--query key=value`, `--format concise|detailed`, `--jq EXPR` and `--template TEXT|@FILE` work on every command.
- Exit codes: 0 ok, 1 failure, 2 usage, 3 refused by the provider, 4 not connected or locked, 130 interrupted.

`chatnip help` prints the full manual, `chatnip help <resource> <verb>` one command with an example. `chatnip commands` lists every command; `chatnip commands --json` adds the input schemas.

## Configs

`~/.chatnip/config.yaml` holds named configs: `app` for your data and `mock` for the mock accounts in `~/.chatnip-mock`. Pick one with `--config mock` or `CHATNIP_CONFIG=mock`, or pin a folder to one with `chatnip config local mock`. `chatnip config get` shows every value and where it came from.

## How it reaches the app

- **The app is open:** the command goes to it over a private socket in Chatnip's data folder. The window updates at once.
- **The app is closed:** the same engine starts without a window, answers and exits.

Either way it runs the same code as the buttons in the app. If the Touch ID lock is on, the command asks for Touch ID first.

## Claude Code plugin

```bash
chatnip plugin install                 # your data
chatnip plugin install --config mock   # the mock accounts
```

This writes `~/.chatnip/plugin` and installs `chatnip@chatnip` through the `claude` command. The plugin holds a skill that explains Chatnip to Claude and `chatnip mcp`, which serves every command as a tool named `<resource>_<verb>`. Tools that send, change or delete something ask you first. `chatnip plugin status` shows what is installed; `chatnip skill install` installs only the skill.

## Other tools

- `chatnip call <engine name> --data '{…}'` runs any command by its engine name.
- `chatnip completion --auto` installs shell completion; `chatnip completion zsh` prints it.
- `chatnip version` prints the version.
