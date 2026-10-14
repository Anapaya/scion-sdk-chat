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
//! The server's TLS identity: a self-signed certificate.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P256_SHA256};
use sha2::{Digest as _, Sha256};

/// The name the certificate is issued for.
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
    /// The certificate file.
    pub cert_path: PathBuf,
    /// The private key file.
    pub key_path: PathBuf,
    /// SHA-256 over the certificate DER, as lower-case hex.
    pub fingerprint: String,
}

impl ServerCert {
    /// Reads the certificate in `data_dir`, generating one the first time.
    pub fn load_or_create(data_dir: &Path) -> Result<ServerCert, CertError> {
        let cert_path = data_dir.join("cert.pem");
        let key_path = data_dir.join("cert.key");

        // Neither half can serve alone, so there is nothing to repair.
        if !cert_path.is_file() || !key_path.is_file() {
            generate(data_dir, &cert_path, &key_path)?;
        }

        Ok(ServerCert {
            fingerprint: fingerprint(&cert_path)?,
            cert_path,
            key_path,
        })
    }
}

fn generate(data_dir: &Path, cert_path: &Path, key_path: &Path) -> Result<(), CertError> {
    fs::create_dir_all(data_dir).map_err(|source| {
        CertError::Io {
            action: "create the data directory",
            path: data_dir.to_owned(),
            source,
        }
    })?;

    // P-256 verifies under every client's default algorithm preferences, which is all the Android
    // SDK accepts.
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = CertificateParams::new(vec![SERVER_NAME.to_owned()])?.self_signed(&key)?;

    write(cert_path, cert.pem().as_bytes(), 0o644)?;
    write(key_path, key.serialize_pem().as_bytes(), 0o600)
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

        let first = ServerCert::load_or_create(dir.path()).expect("a certificate");
        let again = ServerCert::load_or_create(dir.path()).expect("the same certificate");

        assert_eq!(
            first.fingerprint, again.fingerprint,
            "a second start must not invalidate what clients pinned"
        );
    }

    #[test]
    fn a_missing_key_replaces_the_pair() {
        let dir = tempfile::tempdir().expect("a temp dir");

        let first = ServerCert::load_or_create(dir.path()).expect("a certificate");
        fs::remove_file(&first.key_path).expect("removing the key");
        let replaced = ServerCert::load_or_create(dir.path()).expect("a new certificate");

        assert_ne!(
            first.fingerprint, replaced.fingerprint,
            "half a pair cannot serve, so both halves are replaced"
        );
    }

    /// A key any client verifies with its default algorithm preferences.
    #[test]
    fn the_key_is_ecdsa_p256() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let cert = ServerCert::load_or_create(dir.path()).expect("a certificate");

        let key = fs::read_to_string(&cert.key_path).expect("reading the key");
        let der = pem::parse(&key).expect("the key is PEM").into_contents();

        // The OIDs in the PKCS#8 AlgorithmIdentifier: id-ecPublicKey, then prime256v1.
        const EC_PUBLIC_KEY: [u8; 9] = [0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
        const PRIME256V1: [u8; 10] = [0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

        assert!(
            der.windows(EC_PUBLIC_KEY.len())
                .any(|at| at == EC_PUBLIC_KEY),
            "id-ecPublicKey"
        );
        assert!(
            der.windows(PRIME256V1.len()).any(|at| at == PRIME256V1),
            "prime256v1"
        );
    }

    #[test]
    fn the_fingerprint_is_a_sha256_hex_digest() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let cert = ServerCert::load_or_create(dir.path()).expect("a certificate");

        assert_eq!(cert.fingerprint.len(), 64);
        assert!(cert.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
