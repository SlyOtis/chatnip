# Homebrew tap for Chatnip

```bash
brew install --cask slyotis/chatnip/chatnip
```

This installs `Chatnip.app` and puts `chatnip` and `cnip` on your PATH. The cask is updated automatically by each [Chatnip release](https://github.com/SlyOtis/chatnip/releases); its source lives in [`packaging/homebrew`](https://github.com/SlyOtis/chatnip/tree/main/packaging/homebrew) in the main repository, so changes go there.

`brew uninstall --zap --cask chatnip` also removes the local data, `~/.chatnip` and the app's Library folders. Keychain entries stay until you delete them in Keychain Access.
