# Timely Pass CLI

The `timely-pass-cli` is the official command-line interface for the Timely Pass system. It provides a robust suite of tools to manage your secure, time-based credential store directly from your terminal.

## 🚀 Features

- **Full CRUD Operations**: Create, Read, Update, Delete credentials securely.
- **Policy Management**: Associate complex time-based policies with credentials.
- **Secure Defaults**: Automatic secure secret generation and encrypted storage.
- **Cross-Platform**: Runs on Windows, macOS, and Linux.

## 📦 Installation

```bash
cargo install --path .
```

## 🖥️ Usage

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

## 🛡️ Security

- **Visual Privacy**: Secrets are masked by default (`****************`) and only revealed explicitly.
- **Memory Safety**: The CLI leverages the SDK's secure memory handling to ensure secrets don't linger in RAM.

## 📄 License

MIT
