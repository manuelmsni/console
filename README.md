# Console

A tabbed terminal for Windows, written in Rust, built for people who live in the command
line: keep your frequent commands in a tree, reopen your sessions where you left them, and
select and copy text even when the program inside (Claude Code over SSH, for example)
captures the mouse.

## Features

- **Tabs** with cmd or PowerShell. `Ctrl + Shift + T` opens one, middle click or `×` closes
  it, `Ctrl + ←` / `Ctrl + →` switch between them. Double-click a tab to rename it.
- **Saved commands** in a collapsible side panel, organised in folders:
  - `▶` runs a command; clicking its name types it without running it, so you can edit it.
  - Drag and drop to reorder or move into folders; double-click a name to rename it.
  - Filter by name or command text; folders remember whether they were open or folded.
- **Saved tabs**: `Ctrl + S` saves a tab (its screen, with colours, and its command history).
  Saved tabs come back the next time you open the app, even after a crash. Autosave can be
  turned on for every tab (Settings → General) or just for one (`Ctrl + Shift + S`).
- **Per-tab command history**: `Ctrl + ↑` at the prompt lists the commands run in that tab.
  Save any of them as a command, or remove the ones that should not be kept.
- **Copy and paste that work**: select with the mouse and copy with `Ctrl + C` or right click.
  Without a selection, `Ctrl + C` interrupts as usual. Paste with `Ctrl + V`.
- **Links**: `Ctrl + click` opens web links shown in the terminal, even when they wrap over
  several lines.
- **Frameless window** with its own controls. `✕` hides it in the system tray (terminals keep
  running); `⏻` exits. Launching the app again brings back the window that is already open.
- **SSH key helper** (Settings → SSH keys): enable the Windows SSH agent, create keys with a
  passphrase and install them on your servers to log in without typing passwords.
- **Configurable shortcuts** (Settings → Shortcuts).
- **Languages**: English, Spanish and French, following Windows by default.
- **Updates**: when a new version is published, a small button appears next to the window
  controls. Nothing is downloaded until you accept; the download is checked against its
  published SHA-256 checksum before replacing the program. It can be turned off in
  Settings → General. When there is nothing to update, a small coffee cup takes its place: it
  shows which version you have and links to the source code.

## Support

Console is free for personal use and will stay free. If it is useful to you and you want to support its
development, you can [buy me a coffee on GitHub Sponsors](https://github.com/sponsors/manuelmsni).
It is completely voluntary.

## Building

You need [Rust](https://rustup.rs/) (stable, MSVC toolchain).

```
cargo build --release
```

The program is `target\release\console.exe`. It is a single executable with nothing else to install.

### Portable version

```
cargo build --release --features portable --target-dir target\portable
```

The program is `target\portable\release\console.exe`. Copy it anywhere (a USB drive, another
folder or computer): it keeps everything in a `data` folder next to it, encrypted with a
password you choose the first time you open it.

## Where your data is kept

| | Normal version | Portable version |
|---|---|---|
| Folder | `%APPDATA%\console\` | `data\` next to the program |
| Encryption | Your Windows account (DPAPI): only readable by your user on that computer, no password needed | Your password: readable on any computer by whoever knows it |

| File | Contents |
|---|---|
| `commands.json` | Saved commands |
| `settings.json` | Language, autosave and keyboard shortcuts |
| `ssh_servers.json` | SSH server list |
| `tabs\index.json` | Saved tabs and their order |
| `tabs\<id>.journal` | What happened in each saved tab (compacted above 4 MB) |
| `key.json` | Portable only: salt and parameters to check the password |

SSH keys stay in `%USERPROFILE%\.ssh`, where the system's `ssh` looks for them.

### How the portable version protects your data

- A 256-bit key is derived from your password with **Argon2id** (64 MiB of memory, 3 passes),
  which makes guessing passwords slow and expensive.
- Each file is encrypted with **XChaCha20-Poly1305**, which also detects any tampering.
- The password is never stored. If you forget it, the data cannot be recovered.

## Platform support

Windows only for now. Everything that depends on the operating system is isolated in
`src/adapters/platform/`, so other systems can be added by implementing the same small set of
pieces (listed in `src/adapters/platform/mod.rs`).

## Translations

Each language is a JSON file in `locales/` (`en.json` is the reference). To add one, copy
`en.json` (for example to `fr.json`) and translate the values, keeping the keys and the
`{placeholders}` as they are. It is included automatically when building. The tests check that
every language covers every text used by the app.

## Releases

Each release on GitHub contains one executable per kind of build, plus `SHA256SUMS.txt`:

| File | Build |
|---|---|
| `console-windows-x64.exe` | Windows |
| `console-windows-x64-portable.exe` | Windows, portable |

Each copy of the app only updates itself with the file of its own kind, and checks it against
`SHA256SUMS.txt` before installing it.

### Verifying a download

Only download Console from this repository's
[releases page](https://github.com/manuelmsni/console/releases). If you got it from anywhere
else, you can check that it is an official build:

1. **Checksum**: in PowerShell, run

   ```
   Get-FileHash .\console-windows-x64.exe
   ```

   and compare the result with the SHA-256 listed in the notes of the matching release on GitHub
   (renaming the file does not change it). If it differs, do not run it.

2. **Build provenance** (stronger): every file is attested by GitHub as built by this
   repository's release workflow, from a specific commit. With the
   [GitHub CLI](https://cli.github.com/):

   ```
   gh attestation verify .\console-windows-x64.exe --repo manuelmsni/console
   ```

   A checksum copied onto another website could be forged along with the file; this
   attestation cannot.

To publish a version (maintainers):

```
./scripts/release.ps1 1.2.0 -Notes "What changed"
```

It runs the tests, bumps the version, tags the commit and pushes it. GitHub Actions then builds
every kind of executable and publishes the release. No credentials are stored in the
repository.

## License

Console is **source-available, not open source**. See [LICENSE](LICENSE) for the full terms. In short:

- **Personal use is free**: download the official builds from the
  [releases page](https://github.com/manuelmsni/console/releases) and use them on your own devices.
- **Commercial use** (by or for a company or organization, or as part of paid work) requires a
  written license agreement. Contact manuelmsni@hotmail.com to propose one.
- **The source code** can be read and studied, and built unmodified for your own personal use,
  but it may not be modified (except to propose contributions here), redistributed, reused in
  other software, used to build competing products or used to train AI models.
- **Sharing**: please link to this repository instead of sharing copies of the program.

## Development

```
cargo test
cargo test --features portable
```

The code follows a hexagonal architecture (`domain` → `application` → `adapters`).
