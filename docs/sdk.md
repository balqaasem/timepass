# SDK Guide

The `timely-pass-sdk` crate allows you to embed secure, time-based credential management directly into your Rust applications.

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
timely-pass-sdk = { path = "../path/to/timely-pass/timely-pass-sdk" }
# Or once published:
# timely-pass-sdk = "0.1.0"
```

## Basic Usage

### 1. Initialize a Store

```rust
use timely_pass_sdk::store::SecretStore;
use timely_pass_sdk::crypto::Secret;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from("my_store.timely");
    let passphrase = Secret::new(b"my-secure-passphrase".to_vec());

    // Initialize a new store
    let mut store = SecretStore::init(&path, &passphrase)?;
    
    Ok(())
}
```

### 2. Add a Credential

```rust
use timely_pass_sdk::store::{Credential, SecretType};

// ... inside main
let cred = Credential::new(
    "api-key".to_string(),
    SecretType::Key,
    b"super-secret-key-bytes".to_vec()
);

store.add_credential(cred)?;
```

### 3. Read a Credential

```rust
// Open existing store
let store = SecretStore::open(&path, &passphrase)?;

if let Some(cred) = store.get_credential("api-key") {
    println!("Found credential created at: {}", cred.created_at);
    // Access secret data: cred.secret.data
}
```

## Policy Evaluation

You can manually evaluate policies against a context.

```rust
use timely_pass_sdk::policy::{Policy, Period, Hook};
use timely_pass_sdk::eval::{EvaluationContext, Verdict};
use chrono::Utc;

// Create a policy
let policy = Policy {
    id: "working-hours".to_string(),
    version: 1,
    hooks: vec![
        Hook::OnlyWithin(Period::Range {
            start: Utc::now() - chrono::Duration::hours(1),
            end: Utc::now() + chrono::Duration::hours(8),
        })
    ],
    // ... other fields
};

// Create context
let ctx = EvaluationContext {
    now: Utc::now(),
    created_at: Some(Utc::now()),
    last_used_at: None,
    usage_count: 0,
};

// Evaluate
let result = policy.evaluate(&ctx);

if let Verdict::Accept = result.verdict {
    println!("Access Granted");
} else {
    println!("Access Denied: {:?}", result.details);
}
```
