# Security

Chatnip runs only on your Mac. It has no server, and nothing is deployed.

## Where things are kept

- **Passwords, tokens and API keys** are in the macOS Keychain, in a separate entry for each profile.
- **The app registrations' client secrets** are in the Keychain too, in one entry for the app.
- **Development builds** keep secrets in `secrets.dev.json` in `~/Library/Application Support/com.slyotis.chatnip/` instead, readable only by you, because every rebuild of an unsigned dev binary looks like a new app to the Keychain and asks again. They copy each Keychain entry over once. Set `CHATNIP_SECRETS=keychain` to use the Keychain anyway. The installed app always uses the Keychain.
- **Messages, people, labels, rules and events** are in a SQLite database at `~/Library/Application Support/com.slyotis.chatnip/chatnip.db`. The database holds no secrets.
- **Attachments, avatars, preview images and the custom emoji of your Slack workspaces** are files in the same folder.

The database file itself is not encrypted. Turn on FileVault so the disk is.

The app's screens never receive a secret. A saved secret shows as "Saved in Keychain" and can only be replaced.

## Profiles

Each connection belongs to one profile. Lists, search, opening a conversation and background sync only ever see the active profile's data. Profiles never read each other's tokens or messages.

## The lock

Turn it on under **Settings → General → Security**. Chatnip then asks for Touch ID, or your Mac password:
- on launch;
- after the idle time you choose;
- when the screen locks;
- on ⌃⌘L.

While locked:
- only the lock screen shows;
- the app refuses every request except unlocking;
- sync pauses.

## What leaves the Mac

- Calls to the services you connected, each with that connection's own token.
- Calls to the AI you chose: Claude Code, a local Ollama, or an endpoint you set up yourself. These happen only while intelligence is switched on.
- Fetches for the link previews and GIFs you view, and for the custom emoji images of your Slack workspaces.
