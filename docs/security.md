# Security Model

Timely Pass is built with a "Security First" approach. This document outlines the cryptographic primitives, memory safety guarantees, and threat model.

## Cryptographic Primitives

We use only modern, high-security, and widely vetted algorithms.

| Component | Algorithm | Implementation Crate |
|-----------|-----------|----------------------|
| **Encryption** | XChaCha20-Poly1305 | `chacha20poly1305` |
| **Key Derivation** | Argon2id | `argon2` |
| **Randomness** | OS CSPRNG | `rand` / `getrandom` |
| **Zeroization** | - | `zeroize` |

### Key Derivation (KDF)
- **Algorithm**: Argon2id (v13)
- **Parameters**: 
  - Memory: 19 MiB (default)
  - Iterations: 2 (default)
  - Parallelism: 1
- **Salt**: 16-byte random salt, generated per store and stored in the clear in the file header.
- **Purpose**: Derives the 32-byte `MasterKey` from the user's passphrase.

### Encryption
- **Algorithm**: XChaCha20Poly1305 (Extended Nonce ChaCha20-Poly1305).
- **Nonce**: 24-byte random nonce, generated for every write operation.
- **AAD**: The file header (version, salt) is authenticated as Associated Data to prevent header tampering.
- **Key**: The derived `MasterKey`.

## Memory Safety

### Secure Memory Handling
- All sensitive data (passwords, secrets, keys) are stored in `Secret<T>` wrapper types.
- These types derive `Zeroize` and `ZeroizeOnDrop`.
- When a `Secret` goes out of scope, its memory is overwritten with zeros immediately, preventing secrets from lingering in RAM (protection against memory scraping, core dumps).

### Rust Guarantees
- The use of Safe Rust prevents common vulnerabilities like buffer overflows, use-after-free, and double-free errors.

## Storage Format

The store file (`store.timely`) has the following binary structure:

1. **Header Length** (4 bytes, LE u32)
2. **Header** (Bincode serialized):
   - `version` (u32)
   - `salt` (Vec<u8>)
3. **Encrypted Payload**:
   - **Nonce** (24 bytes)
   - **Ciphertext** (XChaCha20Poly1305 output)

The payload decrypts to a `StorePayload` struct containing:
- `credentials`: HashMap<String, Credential>
- `policies`: HashMap<String, Policy>

## Threat Model

### We Defend Against:
- **Offline Attacks**: If an attacker steals the `store.timely` file, they cannot decrypt it without the passphrase (due to Argon2id and strong encryption).
- **Tampering**: Any modification to the file (header or ciphertext) will cause the Poly1305 authentication tag check to fail, alerting the user.
- **Memory Scrapers**: Secrets are zeroed after use, minimizing the window of exposure.

### We Do NOT Defend Against:
- **Keylogging**: If an attacker has a keylogger on your machine, they can capture the passphrase as you type it.
- **Live Memory Analysis**: A sophisticated attacker with root access *while* the program is running might capture secrets in the brief moment they are decrypted.
- **Clock Manipulation**: A user with root access can change the system time to bypass time-based policies. (Future work: Network Time Protocol checks).
