//! Opt-in native OS keyring roundtrip; prepared only, never run automatically.
//!
//! After explicit authorization and unlocking the desktop keyring, run:
//! cargo test --offline --locked -p novel-companion -j2 --test keyring native_keyring_roundtrip_uses_only_its_unique_synthetic_credential -- --ignored --exact
//!
//! This exercises only one fresh random synthetic credential. It does not list
//! credentials, inspect existing peer credentials, enable a listener or use a
//! plaintext fallback. Success establishes this session's store/load/clear
//! roundtrip, not unattended unlock, every desktop/keyring or future availability.
//! Cleanup always attempts to clear this exact identity on ordinary return or
//! panic unwind; abrupt termination or a newly locked keyring can prevent it.

#[allow(dead_code)]
#[path = "../src/sync_service.rs"]
mod sync_service;

use std::{fs::File, io::Read};

fn random_hex() -> String {
    let mut bytes = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .unwrap_or_else(|_| panic!("Operating-system entropy is unavailable; no keyring write attempted."));
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct OwnCredentialCleanup {
    identity: String,
}

impl Drop for OwnCredentialCleanup {
    fn drop(&mut self) {
        // Only this test-owned exact scope is ever cleared. Errors are not printed
        // and cannot expose a bearer token during panic unwind.
        let _ = sync_service::clear_client_token_from_keyring(&self.identity);
    }
}

#[test]
#[ignore = "Requires explicit OS keyring write/read/clear authorization and an unlocked desktop keyring; never execute in the default suite."]
fn native_keyring_roundtrip_uses_only_its_unique_synthetic_credential() {
    let identity = format!("synthetic-keyring-test-{}", random_hex());
    let token = random_hex();
    let _cleanup = OwnCredentialCleanup { identity: identity.clone() };
    assert!(sync_service::store_client_token_in_keyring(&identity, &token).is_ok(),
        "Native synthetic credential store failed; no plaintext fallback used.");
    let loaded = sync_service::load_client_token_from_keyring(&identity)
        .unwrap_or_else(|_| panic!("Native synthetic credential lookup failed."));
    // Avoid assert_eq!: its diagnostic would print both credentials on mismatch.
    assert!(loaded.as_bytes() == token.as_bytes(),
        "Native synthetic credential did not match; values intentionally omitted.");
    assert!(sync_service::clear_client_token_from_keyring(&identity).is_ok(),
        "Native synthetic credential clear failed; only the test scope needs cleanup.");
    assert!(sync_service::load_client_token_from_keyring(&identity).is_err(),
        "Synthetic credential remained readable after clear; value intentionally omitted.");
}
