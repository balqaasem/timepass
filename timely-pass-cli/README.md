# Timely Pass CLI

The `timely-pass-cli` is the official command-line interface for the Timely Pass system. It provides a robust suite of tools to manage your secure, time-based credential store directly from your terminal.

## 🚀 Features

- **Full CRUD Operations**: Create, Read, Update, Delete credentials securely.
- **Interactive TUI**: A full-featured Terminal User Interface for easier management.
- **Policy Management**: Associate complex time-based policies with credentials.
- **Secure Defaults**: Automatic secure secret generation and encrypted storage.
- **Cross-Platform**: Runs on Windows, macOS, and Linux.

## 📦 Installation

```bash
cargo install --path .
```

## 🖥️ Usage

### Command Line Interface (CLI)

```bash
# Initialize a new store
timely-pass init

# Add a new credential
timely-pass add --id my-secret --type password --secret

# Get a credential (subject to policy)
timely-pass get --id my-secret

# List all credentials
timely-pass list

# Remove a credential
timely-pass remove --id my-secret
```

### Terminal User Interface (TUI)

Launch the interactive mode:

```bash
timely-pass tui
```

#### Navigation & Global Keys

| Key | Action |
| :--- | :--- |
| `1` | Switch to **Credentials** Tab |
| `2` | Switch to **Policies** Tab |
| `3` | Switch to **Audit Log** Tab |
| `q` | Quit application |

#### Credentials Tab (Tab 1)

| Key | Action |
| :--- | :--- |
| `a` | **Add** a new credential (opens popup) |
| `d` / `Delete` | **Delete** selected credential (opens confirmation) |
| `r` | **Rotate** secret for selected credential |
| `/` | **Search** / Filter list by ID |
| `c` | **Copy** secret to clipboard (securely) |
| `Enter` | **Reveal/Hide** secret details |
| `E` | **Export** selected credential to JSON |

#### Policies Tab (Tab 2)

| Key | Action |
| :--- | :--- |
| `a` | **Add** a new policy (opens wizard) |
| `d` / `Delete` | **Delete** selected policy |
| `e` | **Enable/Disable** selected policy |
| `Enter` | **Edit** selected policy |
| `/` | **Search** / Filter policies |
| `E` | **Export** selected policy to JSON |

#### Audit Log Tab (Tab 3)

| Key | Action |
| :--- | :--- |
| `/` | **Search** / Filter logs |
| `E` | **Export** full audit log to JSON |


## 🛡️ Security

- **Clipboard Protection**: Secrets copied to the clipboard are handled securely.
- **Visual Privacy**: Secrets are masked by default (`****************`) and only revealed explicitly.
- **Memory Safety**: The CLI leverages the SDK's secure memory handling to ensure secrets don't linger in RAM.

## 📄 License

MIT
