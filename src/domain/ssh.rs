//! Servidores SSH y claves para entrar en ellos sin contraseña.
//!
//! La aplicación no guarda contraseñas ni frases de paso: usa las claves
//! estándar de OpenSSH (las de `%USERPROFILE%\.ssh`), protegidas con su frase
//! de paso, y el agente SSH de Windows para no tener que escribirla siempre.
//! Todo lo que pide una frase de paso se hace en una pestaña, con las propias
//! herramientas de OpenSSH, para que nunca pase por la app.

use std::fmt;

use crate::i18n::tr;

/// A dónde conectarse: `usuario@servidor`, o solo `servidor`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SshDestination(String);

/// Nombre del fichero de una clave dentro de `.ssh`, por ejemplo `id_ed25519`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SshKeyName(String);

/// Una clave encontrada en `.ssh`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshKeyInfo {
    pub name: SshKeyName,
    /// Por ejemplo `ssh-ed25519`.
    pub key_type: String,
    /// Normalmente `usuario@equipo` de quien la creó.
    pub comment: String,
    /// La línea completa de la clave pública, la que se copia a los servidores.
    pub public_key: String,
    /// Sin frase de paso, quien copie el fichero puede usar la clave.
    pub has_passphrase: bool,
    /// Cargada en el agente SSH: se usa sin escribir la frase de paso.
    pub loaded_in_agent: bool,
}

/// Qué pasa al intentar entrar en un servidor sin escribir nada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginCheck {
    EntersWithoutAsking,
    /// El servidor acepta la clave, pero está protegida con frase de paso y no
    /// está cargada en el agente: `ssh` la pedirá en cada conexión.
    KeyAcceptedButLocked,
    /// Pide contraseña, la clave no está instalada, o el servidor no responde.
    DoesNotEnter,
}

/// El servicio "OpenSSH Authentication Agent" de Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SshAgentStatus {
    Running,
    /// Viene así de serie en Windows; activarlo pide permiso de administrador.
    Stopped,
    NotInstalled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SshInputError {
    Empty,
    /// Solo se aceptan letras, números y `. _ - @ :` para que el texto no
    /// pueda colarse en un comando como otra orden.
    UnsafeCharacters,
}

impl fmt::Display for SshInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&match self {
            SshInputError::Empty => tr("ssh.error.empty"),
            SshInputError::UnsafeCharacters => tr("ssh.error.unsafe_characters"),
        })
    }
}

impl SshDestination {
    pub fn parse(text: &str) -> Result<Self, SshInputError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(SshInputError::Empty);
        }
        if !text.chars().all(|character| character.is_ascii_alphanumeric() || "._-@:".contains(character)) {
            return Err(SshInputError::UnsafeCharacters);
        }
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SshKeyName {
    pub fn parse(text: &str) -> Result<Self, SshInputError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(SshInputError::Empty);
        }
        let is_safe = text.chars().all(|character| character.is_ascii_alphanumeric() || "._-".contains(character));
        if !is_safe || text.starts_with('.') || text.ends_with(".pub") {
            return Err(SshInputError::UnsafeCharacters);
        }
        Ok(Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Comando de cmd que añade la clave pública al servidor. Pide la contraseña
/// del servidor una última vez; después ya se entra con la clave.
pub fn install_public_key_command(public_key_file: &str, destination: &SshDestination) -> String {
    let add_to_authorized_keys =
        "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys";
    format!("type \"{public_key_file}\" | ssh {} \"{add_to_authorized_keys}\"", destination.as_str())
}

pub fn connect_command(destination: &SshDestination) -> String {
    format!("ssh {}", destination.as_str())
}

/// Crea una clave nueva. `ssh-keygen` pide la frase de paso dos veces, sin mostrarla.
pub fn generate_key_command(private_key_file: &str, comment: &str) -> String {
    format!("ssh-keygen -t ed25519 -C \"{}\" -f \"{private_key_file}\"", safe_key_comment(comment))
}

/// Pone (o cambia) la frase de paso de una clave que ya existe.
pub fn change_passphrase_command(private_key_file: &str) -> String {
    format!("ssh-keygen -p -f \"{private_key_file}\"")
}

/// Carga la clave en el agente: pide la frase de paso una vez y Windows la
/// recuerda, también después de reiniciar.
pub fn add_to_agent_command(private_key_file: &str) -> String {
    format!("ssh-add \"{private_key_file}\"")
}

/// El comentario va dentro de un comando: solo se dejan caracteres inofensivos.
fn safe_key_comment(comment: &str) -> String {
    comment.chars().filter(|character| character.is_ascii_alphanumeric() || "@._-".contains(*character)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_usual_destinations() {
        assert!(SshDestination::parse("usuario@servidor.local").is_ok());
        assert!(SshDestination::parse("192.168.1.20").is_ok());
        assert_eq!(SshDestination::parse("  pi  ").unwrap().as_str(), "pi");
    }

    #[test]
    fn rejects_destinations_that_could_run_other_commands() {
        assert_eq!(SshDestination::parse("pi & del *"), Err(SshInputError::UnsafeCharacters));
        assert_eq!(SshDestination::parse("pi\"|calc"), Err(SshInputError::UnsafeCharacters));
        assert_eq!(SshDestination::parse("   "), Err(SshInputError::Empty));
    }

    #[test]
    fn key_names_must_be_plain_file_names() {
        assert!(SshKeyName::parse("id_ed25519").is_ok());
        assert_eq!(SshKeyName::parse("..\\otra"), Err(SshInputError::UnsafeCharacters));
        assert_eq!(SshKeyName::parse("clave.pub"), Err(SshInputError::UnsafeCharacters));
    }

    #[test]
    fn key_comment_cannot_break_out_of_its_quotes() {
        let command = generate_key_command(r"C:\Users\yo\.ssh\id_ed25519", "yo@PC\" & calc &\"");
        assert_eq!(command, r#"ssh-keygen -t ed25519 -C "yo@PCcalc" -f "C:\Users\yo\.ssh\id_ed25519""#);
    }

    #[test]
    fn install_command_sends_the_public_key_to_the_server() {
        let destination = SshDestination::parse("dev@pi").unwrap();
        let command = install_public_key_command(r"C:\Users\yo\.ssh\id_ed25519.pub", &destination);
        assert!(command.starts_with(r#"type "C:\Users\yo\.ssh\id_ed25519.pub" | ssh dev@pi "#));
        assert!(command.contains("authorized_keys"));
    }
}
