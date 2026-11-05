//! Cifrado ligado a la cuenta de Windows (DPAPI): lo cifrado solo se puede
//! descifrar con este mismo usuario en este mismo equipo. No hay contraseña
//! aparte que recordar ni que perder.

use std::ptr;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

use crate::adapters::file_cipher::FileCipher;
use crate::i18n::tr;

/// Dato extra propio de esta app: otro programa del mismo usuario que llame
/// a DPAPI sin conocerlo no puede descifrar los ficheros directamente.
const APP_ENTROPY: &[u8] = b"consola: configuracion v1";

/// Marca al principio de los ficheros cifrados así, para distinguirlos de los
/// de versiones anteriores, que se guardaban en texto plano.
const FILE_MARK: &[u8] = b"CONSOLA-DPAPI-1\n";

/// Cifrado ligado a la cuenta de Windows: sin contraseña, pero solo sirve en
/// este equipo y con este usuario.
pub struct WindowsUserCipher;

impl FileCipher for WindowsUserCipher {
    fn file_mark(&self) -> &'static [u8] {
        FILE_MARK
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        encrypt_for_current_user(plain)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        decrypt_for_current_user(encrypted)
    }
}

fn encrypt_for_current_user(plain: &[u8]) -> Result<Vec<u8>, String> {
    // Seguro: los blobs apuntan a datos vivos durante la llamada y la salida
    // se copia y se libera con LocalFree, como pide la documentación de Windows.
    unsafe {
        let input = blob_of(plain);
        let entropy = blob_of(APP_ENTROPY);
        let mut output = empty_blob();
        let succeeded =
            CryptProtectData(&input, ptr::null(), &entropy, ptr::null(), ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output);
        if succeeded == 0 {
            return Err(tr("encryption.error.encrypt"));
        }
        Ok(take_blob(output))
    }
}

fn decrypt_for_current_user(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    // Seguro: igual que en `encrypt_for_current_user`.
    unsafe {
        let input = blob_of(encrypted);
        let entropy = blob_of(APP_ENTROPY);
        let mut output = empty_blob();
        let succeeded = CryptUnprotectData(
            &input,
            ptr::null_mut(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if succeeded == 0 {
            return Err(tr("encryption.error.decrypt"));
        }
        Ok(take_blob(output))
    }
}

fn blob_of(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

fn empty_blob() -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() }
}

/// Copia el resultado que reservó Windows y libera su memoria.
unsafe fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec();
    LocalFree(blob.pbData as _);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_data_comes_back_the_same() {
        let secret = "historial con algo delicado".as_bytes();
        let encrypted = encrypt_for_current_user(secret).unwrap();
        assert_ne!(encrypted, secret);
        assert_eq!(decrypt_for_current_user(&encrypted).unwrap(), secret);
    }
}
