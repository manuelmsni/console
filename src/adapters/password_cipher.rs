//! Cifrado con contraseña, para la versión portable: los ficheros se pueden
//! llevar a otro equipo y abrirlos allí sabiendo la contraseña.
//!
//! - De la contraseña se saca una clave de 256 bits con Argon2id, que gasta a
//!   propósito mucha memoria y tiempo: probar contraseñas a lo bruto sale caro.
//! - Con esa clave se cifra cada fichero con XChaCha20-Poly1305, que además
//!   detecta si alguien ha tocado el fichero.
//! - La contraseña no se guarda en ningún sitio. En `key.json` solo van la
//!   "sal" (un valor al azar para que dos contraseñas iguales den claves
//!   distintas), los parámetros de Argon2 y un texto cifrado de comprobación
//!   para saber si la contraseña escrita es la buena.

use std::fs;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::file_cipher::FileCipher;
use crate::i18n::{tr, tr_args};

const FILE_MARK: &[u8] = b"CONSOLA-CLAVE-1\n";
const KEY_FILE_NAME: &str = "key.json";
const NONCE_SIZE: usize = 24;
const SALT_SIZE: usize = 16;
const KEY_SIZE: usize = 32;
/// Lo que se cifra en `key.json` para comprobar la contraseña.
const CHECK_TEXT: &[u8] = b"consola portable: la clave es correcta";

/// Cuánto le cuesta a Argon2 sacar la clave. Se guardan en `key.json`, así
/// que se pueden subir en el futuro sin dejar de abrir los datos antiguos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDerivationCost {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KeyDerivationCost {
    /// Lo recomendado para guardar contraseñas: 64 MiB y 3 pasadas (menos de
    /// un segundo en un equipo normal, una vez al abrir la aplicación).
    pub const RECOMMENDED: Self = Self { memory_kib: 64 * 1024, iterations: 3, parallelism: 1 };
}

pub struct PasswordCipher {
    cipher: XChaCha20Poly1305,
}

impl PasswordCipher {
    fn from_password(password: &str, salt: &[u8], cost: KeyDerivationCost) -> Result<Self, String> {
        let params = Params::new(cost.memory_kib, cost.iterations, cost.parallelism, Some(KEY_SIZE))
            .map_err(|error| error.to_string())?;
        let mut key = Zeroizing::new([0u8; KEY_SIZE]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.as_bytes(), salt, key.as_mut())
            .map_err(|error| error.to_string())?;
        Ok(Self { cipher: XChaCha20Poly1305::new(Key::from_slice(key.as_ref())) })
    }
}

impl FileCipher for PasswordCipher {
    fn file_mark(&self) -> &'static [u8] {
        FILE_MARK
    }

    /// Un número de un solo uso al azar delante y, detrás, lo cifrado.
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let encrypted = self.cipher.encrypt(&nonce, plain).map_err(|_| tr("encryption.error.encrypt_with_password"))?;
        Ok([nonce.as_slice(), &encrypted].concat())
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted.len() < NONCE_SIZE {
            return Err(tr("encryption.error.decrypt_with_password"));
        }
        let (nonce, encrypted) = encrypted.split_at(NONCE_SIZE);
        self.cipher.decrypt(XNonce::from_slice(nonce), encrypted).map_err(|_| tr("encryption.error.decrypt_with_password"))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UnlockError {
    WrongPassword,
    /// `key.json` no se puede leer o está dañado.
    KeyFileUnreadable(String),
}

/// `key.json`, en la carpeta de datos portable.
#[derive(Serialize, Deserialize)]
struct StoredKeyFile {
    version: u32,
    algorithm: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: String,
    check: String,
}

#[derive(Clone)]
pub struct PasswordKeyFile {
    path: PathBuf,
}

impl PasswordKeyFile {
    pub fn in_folder(folder: &Path) -> Self {
        Self { path: folder.join(KEY_FILE_NAME) }
    }

    /// Si no existe, es la primera vez: hay que pedir que se cree la contraseña.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn create(&self, password: &str, cost: KeyDerivationCost) -> Result<PasswordCipher, String> {
        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        let cipher = PasswordCipher::from_password(password, &salt, cost)?;
        let stored = StoredKeyFile {
            version: 1,
            algorithm: "argon2id+xchacha20poly1305".to_string(),
            memory_kib: cost.memory_kib,
            iterations: cost.iterations,
            parallelism: cost.parallelism,
            salt: hex_from(&salt),
            check: hex_from(&cipher.encrypt(CHECK_TEXT)?),
        };
        let json = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder)
                .map_err(|error| tr_args("storage.error.create_folder", &[("path", &folder.display()), ("error", &error)]))?;
        }
        fs::write(&self.path, json).map_err(|error| tr_args("storage.error.save", &[("path", &self.path.display()), ("error", &error)]))?;
        Ok(cipher)
    }

    pub fn unlock(&self, password: &str) -> Result<PasswordCipher, UnlockError> {
        let unreadable = |error: String| UnlockError::KeyFileUnreadable(tr_args("storage.error.read", &[("path", &self.path.display()), ("error", &error)]));
        let json = fs::read_to_string(&self.path).map_err(|error| unreadable(error.to_string()))?;
        let stored: StoredKeyFile = serde_json::from_str(&json).map_err(|error| unreadable(error.to_string()))?;
        let salt = bytes_from_hex(&stored.salt).ok_or_else(|| unreadable("salt".to_string()))?;
        let check = bytes_from_hex(&stored.check).ok_or_else(|| unreadable("check".to_string()))?;
        let cost = KeyDerivationCost { memory_kib: stored.memory_kib, iterations: stored.iterations, parallelism: stored.parallelism };

        let cipher = PasswordCipher::from_password(password, &salt, cost).map_err(unreadable)?;
        match cipher.decrypt(&check) {
            Ok(text) if text == CHECK_TEXT => Ok(cipher),
            _ => Err(UnlockError::WrongPassword),
        }
    }
}

fn hex_from(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn bytes_from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|start| u8::from_str_radix(text.get(start..start + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Barato, para que los tests vayan rápido. La aplicación usa `RECOMMENDED`.
    const CHEAP: KeyDerivationCost = KeyDerivationCost { memory_kib: 64, iterations: 1, parallelism: 1 };

    fn empty_test_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("console-tests-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn the_right_password_opens_what_was_saved() {
        let folder = empty_test_folder("buena");
        let key_file = PasswordKeyFile::in_folder(&folder);
        assert!(!key_file.exists());

        let encrypted = key_file.create("caballo grapa batería", CHEAP).unwrap().encrypt(b"ssh dev@servidor").unwrap();
        assert!(key_file.exists());

        let reopened = key_file.unlock("caballo grapa batería").unwrap();
        assert_eq!(reopened.decrypt(&encrypted).unwrap(), b"ssh dev@servidor");
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_wrong_password_is_rejected() {
        let folder = empty_test_folder("mala");
        let key_file = PasswordKeyFile::in_folder(&folder);
        key_file.create("caballo grapa batería", CHEAP).unwrap();

        assert!(matches!(key_file.unlock("caballo grapa"), Err(UnlockError::WrongPassword)));
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn the_password_is_not_written_anywhere() {
        let folder = empty_test_folder("secreta");
        PasswordKeyFile::in_folder(&folder).create("contraseña-muy-secreta", CHEAP).unwrap();
        let key_file_text = fs::read_to_string(folder.join(KEY_FILE_NAME)).unwrap();

        assert!(!key_file_text.contains("contraseña-muy-secreta"));
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_tampered_file_is_detected() {
        let folder = empty_test_folder("tocado");
        let cipher = PasswordKeyFile::in_folder(&folder).create("caballo grapa batería", CHEAP).unwrap();
        let mut encrypted = cipher.encrypt(b"comandos").unwrap();
        *encrypted.last_mut().unwrap() ^= 1;

        assert!(cipher.decrypt(&encrypted).is_err());
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn hex_goes_and_comes_back() {
        assert_eq!(bytes_from_hex(&hex_from(&[0, 15, 255])).unwrap(), vec![0, 15, 255]);
        assert_eq!(bytes_from_hex("abc"), None);
        assert_eq!(bytes_from_hex("zz"), None);
    }
}