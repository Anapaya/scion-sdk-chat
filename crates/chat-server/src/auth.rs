// Copyright 2026 Anapaya Systems
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Password hashing and bearer tokens.

use std::{
    path::Path,
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, Salt, SaltString},
};
use chat_core::api::v1::UnixMillis;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::store::PasswordHash;

/// A hash of nothing anyone can log in with, verified against when the account does not exist so
/// that a caller cannot tell a wrong password from an unknown name by timing the response.
static ABSENT_ACCOUNT: LazyLock<PasswordHash> = LazyLock::new(|| {
    hash_password("a password no account has").expect("hashing a constant cannot fail")
});

/// Anything the auth layer can fail with. Deliberately coarse: a caller must not be able to tell
/// these apart, so the API reports one `invalid_credentials` for all of them.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Hashing or verification failed.
    #[error("password: {0}")]
    Password(argon2::password_hash::Error),
    /// Signing or decoding a token failed.
    #[error("token: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    /// The secret file could not be read or written.
    #[error("jwt secret {path}: {source}")]
    Secret {
        /// The file being read or written.
        path: std::path::PathBuf,
        /// What the filesystem reported.
        source: std::io::Error,
    },
}

/// What a token carries: who it is, and when it stops being accepted.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// The username the token was issued to.
    pub sub: String,
    /// Expiry, as seconds since the Unix epoch — the representation JWT specifies.
    pub exp: u64,
}

/// Hashes a password with Argon2id and a fresh random salt.
///
/// Two calls with the same password produce different strings, which is why verification reads
/// the stored hash rather than recomputing and comparing.
pub fn hash_password(password: &str) -> Result<PasswordHash, AuthError> {
    let salt = SaltString::encode_b64(&rand::random::<[u8; Salt::RECOMMENDED_LENGTH]>())
        .map_err(AuthError::Password)?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(AuthError::Password)?;

    Ok(PasswordHash::new(hash.to_string()))
}

/// Checks a password against a stored hash, in constant time.
///
/// `stored` is `None` when the account does not exist. The check still runs, against a fixed
/// hash, so that the answer takes the same time either way and reveals nothing about which
/// usernames are registered.
pub fn verify_password(password: &str, stored: Option<&PasswordHash>) -> bool {
    let (hash, exists) = match stored {
        Some(hash) => (hash, true),
        None => (&*ABSENT_ACCOUNT, false),
    };

    let Ok(parsed) = argon2::PasswordHash::new(hash.as_str()) else {
        return false;
    };
    let verified = Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();

    verified && exists
}

/// The HS256 secret, read from `path` or generated and written there on first start.
///
/// Losing the file logs everyone out and costs nothing else.
pub fn load_or_create_secret(path: &Path) -> Result<Vec<u8>, AuthError> {
    let fail = |source| {
        AuthError::Secret {
            path: path.to_path_buf(),
            source,
        }
    };

    match std::fs::read(path) {
        Ok(secret) => Ok(secret),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let secret: [u8; 32] = rand::random();
            write_private(path, &secret).map_err(fail)?;
            Ok(secret.to_vec())
        }
        Err(e) => Err(fail(e)),
    }
}

/// Writes a file only the owner can read.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        std::io::Write::write_all(&mut file, bytes)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes)
    }
}

/// Signs and validates the bearer tokens issued at login.
#[derive(Clone)]
pub struct Tokens {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validity: Duration,
}

impl Tokens {
    /// Builds the signer from a secret and the lifetime tokens are issued with.
    pub fn new(secret: &[u8], validity: Duration) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            validity,
        }
    }

    /// Issues a token for `username`, and reports when it expires.
    pub fn issue(&self, username: &str) -> Result<(String, UnixMillis), AuthError> {
        let expires_at = now() + self.validity;
        let claims = Claims {
            sub: username.to_owned(),
            exp: expires_at.as_secs(),
        };
        let token = jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)?;

        Ok((token, UnixMillis::new(expires_at.as_millis() as u64)))
    }

    /// The username a token was issued to, if the signature holds and it has not expired.
    pub fn verify(&self, token: &str) -> Result<String, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        // The default allows 60s of clock skew between issuer and verifier. The same process does
        // both here, so the only thing that buys is a minute of accepting expired tokens.
        validation.leeway = 0;

        let data = jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)?;

        Ok(data.claims.sub)
    }
}

fn now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is set before the Unix epoch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash_and_nothing_else() {
        let hash = hash_password("correct horse battery staple").expect("hash");

        assert!(verify_password("correct horse battery staple", Some(&hash)));
        assert!(!verify_password("wrong", Some(&hash)));
    }

    #[test]
    fn hashing_the_same_password_twice_gives_different_hashes() {
        let first = hash_password("same").expect("hash");
        let second = hash_password("same").expect("hash");

        assert_ne!(
            first.as_str(),
            second.as_str(),
            "the salt should differ per hash, which is why login cannot compare hashes"
        );
        assert!(verify_password("same", Some(&second)));
    }

    #[test]
    fn an_unknown_account_still_runs_a_verification() {
        // The point is that it answers `false` rather than short-circuiting, so the two failure
        // paths take comparable time. Nothing here can assert timing, but the shape is the fix.
        assert!(!verify_password("anything", None));
    }

    #[test]
    fn a_token_round_trips_and_a_tampered_one_does_not() {
        let tokens = Tokens::new(b"secret", Duration::from_secs(60));
        let (token, expires_at) = tokens.issue("alice").expect("issue");

        assert_eq!(tokens.verify(&token).expect("verify"), "alice");
        assert!(expires_at.get() > 0);
        assert!(tokens.verify(&format!("{token}x")).is_err());
    }

    #[test]
    fn a_token_signed_with_another_secret_is_refused() {
        let ours = Tokens::new(b"ours", Duration::from_secs(60));
        let theirs = Tokens::new(b"theirs", Duration::from_secs(60));
        let (token, _) = theirs.issue("alice").expect("issue");

        assert!(ours.verify(&token).is_err());
    }

    /// Ten seconds past would still be accepted under jsonwebtoken's default 60s of leeway, so
    /// this covers `verify` setting it to zero as much as it covers expiry itself.
    #[test]
    fn an_expired_token_is_refused() {
        let tokens = Tokens::new(b"secret", Duration::from_secs(60));
        let expired = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &Claims {
                sub: "alice".to_owned(),
                exp: now().as_secs() - 10,
            },
            &EncodingKey::from_secret(b"secret"),
        )
        .expect("encode");

        assert!(tokens.verify(&expired).is_err());
    }
}
