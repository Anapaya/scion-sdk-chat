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
//! The server's TLS identity: a self-signed certificate in the data directory.
//!
//! Generated once and kept. Clients pin this certificate, so a new one on every start would lock
//! out everyone already holding the old fingerprint.
//!
//! Self-signed because a private deployment has no public reachability: there is no ACME challenge
//! to answer, and no public authority that could issue for it.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use rcgen::{CertificateParams, KeyPair, PKCS_ED25519};
use sha2::{Digest as _, Sha256};

/// The name the certificate is issued for.
///
/// Clients reach the server at `https://localhost:<port>` and say where to send the packets
/// separately, so the URL's host is what has to match here, not anything routable.
pub const SERVER_NAME: &str = "localhost";

/// Anything that stops the certificate from being ready.
#[derive(Debug, thiserror::Error)]
pub enum CertError {
    /// A file could not be read or written.
    #[error("could not {action} {path}: {source}")]
    Io {
        /// What was being attempted, for a message that says which file and why.
        action: &'static str,
        /// The file involved.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
    /// The certificate could not be generated.
    #[error("generating a self-signed certificate: {0}")]
    Generate(#[from] rcgen::Error),
    /// The stored certificate is not readable PEM.
    #[error("the certificate at {path} is not readable PEM: {source}")]
    Parse {
        /// The file involved.
        path: PathBuf,
        /// What the decoder reported.
        source: pem::PemError,
    },
}

/// A certificate ready to serve with, and what to tell clients to pin.
#[derive(Debug, Clone)]
pub struct ServerCert {
    /// The certificate, as a file because that is the only way squiche will load one.
    pub cert_path: PathBuf,
    /// The private key, likewise.
    pub key_path: PathBuf,
    /// SHA-256 over the DER, lower-case hex — the same digest `openssl x509 -fingerprint` prints.
    pub fingerprint: String,
}

/// Reads the certificate in `data_dir`, generating one the first time.
///
/// A half-written pair, one file without the other, is replaced rather than repaired: there is
/// nothing to serve with either half alone.
pub fn load_or_create(data_dir: &Path) -> Result<ServerCert, CertError> {
    let cert_path = data_dir.join("cert.pem");
    let key_path = data_dir.join("cert.key");

    if !cert_path.is_file() || !key_path.is_file() {
        generate(data_dir, &cert_path, &key_path)?;
    }

    Ok(ServerCert {
        fingerprint: fingerprint(&cert_path)?,
        cert_path,
        key_path,
    })
}

fn generate(data_dir: &Path, cert_path: &Path, key_path: &Path) -> Result<(), CertError> {
    fs::create_dir_all(data_dir).map_err(|source| {
        CertError::Io {
            action: "create the data directory",
            path: data_dir.to_owned(),
            source,
        }
    })?;

    let key_pem = ed25519_pkcs8_v1(rand::random());
    let key = KeyPair::from_pkcs8_pem_and_sign_algo(&key_pem, &PKCS_ED25519)?;
    let cert = CertificateParams::new(vec![SERVER_NAME.to_owned()])?.self_signed(&key)?;

    write(cert_path, cert.pem().as_bytes(), 0o644)?;
    // The PEM built above rather than `key.serialize_pem()`, so the file keeps the v1 encoding.
    write(key_path, key_pem.as_bytes(), 0o600)
}

/// An Ed25519 private key as PKCS#8 **v1** PEM, which is the version BoringSSL will sign with.
///
/// `rcgen::KeyPair::generate_for(&PKCS_ED25519)` cannot be used: it delegates to ring, which emits
/// v2 — the same key plus its public half — and squiche then fails the handshake. The structure is
/// fixed at 48 bytes, so it is written out rather than reached for a DER encoder (RFC 8410 §7).
fn ed25519_pkcs8_v1(seed: [u8; 32]) -> String {
    const PREFIX: [u8; 16] = [
        0x30, 0x2e, // SEQUENCE, 46 bytes
        0x02, 0x01, 0x00, // INTEGER 0, the version that omits the public key
        0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, // AlgorithmIdentifier, Ed25519
        0x04, 0x22, 0x04, 0x20, // OCTET STRING wrapping the 32-byte seed
    ];

    let mut der = Vec::with_capacity(PREFIX.len() + seed.len());
    der.extend_from_slice(&PREFIX);
    der.extend_from_slice(&seed);

    pem::encode(&pem::Pem::new("PRIVATE KEY", der))
}

fn fingerprint(cert_path: &Path) -> Result<String, CertError> {
    let text = fs::read(cert_path).map_err(|source| {
        CertError::Io {
            action: "read the certificate",
            path: cert_path.to_owned(),
            source,
        }
    })?;
    let block = pem::parse(&text).map_err(|source| {
        CertError::Parse {
            path: cert_path.to_owned(),
            source,
        }
    })?;

    // Over the DER, not the PEM, to match what every other tool reports.
    Ok(Sha256::digest(block.contents())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn write(path: &Path, contents: &[u8], mode: u32) -> Result<(), CertError> {
    let failed = |source| {
        CertError::Io {
            action: "write",
            path: path.to_owned(),
            source,
        }
    };

    #[cfg(unix)]
    {
        use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};

        // Set as the file is created, so the key is never briefly world-readable.
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(path)
            .and_then(|mut file| file.write_all(contents))
            .map_err(failed)
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
        fs::write(path, contents).map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_certificate_is_generated_once_and_then_reused() {
        let dir = tempfile::tempdir().expect("a temp dir");

        let first = load_or_create(dir.path()).expect("a certificate");
        let again = load_or_create(dir.path()).expect("the same certificate");

        assert_eq!(
            first.fingerprint, again.fingerprint,
            "a second start must not invalidate what clients pinned"
        );
    }

    #[test]
    fn a_missing_key_replaces_the_pair() {
        let dir = tempfile::tempdir().expect("a temp dir");

        let first = load_or_create(dir.path()).expect("a certificate");
        fs::remove_file(&first.key_path).expect("removing the key");
        let replaced = load_or_create(dir.path()).expect("a new certificate");

        assert_ne!(
            first.fingerprint, replaced.fingerprint,
            "half a pair cannot serve, so both halves are replaced"
        );
    }

    /// Ed25519, and version 1. A v2 key holds the public half as well, and squiche will not sign
    /// with one. See [`ed25519_pkcs8_v1`].
    #[test]
    fn the_key_is_ed25519_in_the_version_boringssl_signs_with() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let cert = load_or_create(dir.path()).expect("a certificate");

        let key = fs::read_to_string(&cert.key_path).expect("reading the key");
        let der = pem::parse(&key).expect("the key is PEM").into_contents();

        assert_eq!(der.len(), 48, "a v1 Ed25519 key is 48 bytes; v2 is 83");
        assert_eq!(
            &der[5..12],
            &[0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70],
            "Ed25519"
        );
        assert_eq!(der[4], 0, "version 0 is PKCS#8 v1");
    }

    #[test]
    fn the_fingerprint_is_a_sha256_hex_digest() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let cert = load_or_create(dir.path()).expect("a certificate");

        assert_eq!(cert.fingerprint.len(), 64);
        assert!(cert.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
