//! Cómo se cifran los ficheros que guarda la aplicación. Hay dos formas:
//!
//! - Instalada: con la cuenta del usuario del sistema (en Windows, DPAPI). No
//!   hay contraseña, pero los ficheros solo sirven en ese equipo y con ese usuario.
//! - Portable: con una contraseña que pone el usuario (ver `password_cipher`).
//!   Los ficheros sirven en cualquier equipo, sabiendo la contraseña.

pub trait FileCipher: Send + Sync {
    /// Lo que va al principio de cada fichero cifrado así, para reconocerlo.
    fn file_mark(&self) -> &'static [u8];
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, String>;
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String>;
}