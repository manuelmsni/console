//! Actualizaciones: comparar versiones y elegir, de una release publicada, el
//! fichero que corresponde a este programa.
//!
//! Cada release lleva un ejecutable por tipo de compilación, siempre con el
//! mismo nombre (ver `BuildKind::release_file_name`), y un `SHA256SUMS.txt`
//! con la suma de cada uno. Así cada programa sabe cuál es el suyo: la versión
//! portable de Windows solo se actualiza con la portable de Windows, etc.

use std::cmp::Ordering;
use std::fmt;

/// Fichero de la release con la suma SHA-256 de cada ejecutable.
pub const CHECKSUMS_FILE_NAME: &str = "SHA256SUMS.txt";

/// `1.4.2`. En las etiquetas de GitHub va con una `v` delante (`v1.4.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Acepta `1.4.2` y `v1.4.2`. Lo que vaya tras un `-` o `+` (como en
    /// `1.4.2-beta`) no se admite: solo se ofrecen versiones finales.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let numbers = text.strip_prefix('v').unwrap_or(text);
        let mut parts = numbers.split('.').map(|part| part.parse::<u32>().ok());
        let version = Self { major: parts.next()??, minor: parts.next()??, patch: parts.next()?? };
        parts.next().is_none().then_some(version)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatingSystem {
    Windows,
    /// Prevista: se usará cuando exista `adapters/platform/linux`.
    #[allow(dead_code)]
    Linux,
}

/// Qué programa es este: para qué sistema y si es la versión portable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildKind {
    pub operating_system: OperatingSystem,
    pub is_portable: bool,
}

impl BuildKind {
    /// `console-windows-x64.exe`, `console-windows-x64-portable.exe`,
    /// `console-linux-x64`, `console-linux-x64-portable`. Los mismos nombres
    /// que pone el flujo de publicación (`.github/workflows/release.yml`).
    pub fn release_file_name(self) -> String {
        let (system, extension) = match self.operating_system {
            OperatingSystem::Windows => ("windows", ".exe"),
            OperatingSystem::Linux => ("linux", ""),
        };
        let portable = if self.is_portable { "-portable" } else { "" };
        format!("console-{system}-x64{portable}{extension}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseFile {
    pub name: String,
    pub download_url: String,
}

/// La última release publicada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Lo que ha cambiado, tal como se escribió al publicarla.
    pub notes: String,
    pub web_page: String,
    pub files: Vec<ReleaseFile>,
}

/// Una actualización que se puede ofrecer a este programa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOffer {
    pub version: Version,
    pub notes: String,
    pub web_page: String,
    pub program: ReleaseFile,
    pub checksums: ReleaseFile,
}

/// Solo se ofrece si es más nueva que la actual, no se ha pedido saltarla y
/// trae el ejecutable de este tipo de programa (y sus sumas para comprobarlo).
pub fn update_to_offer(current: Version, kind: BuildKind, latest: &Release, skipped: Option<Version>) -> Option<UpdateOffer> {
    if latest.version <= current || skipped == Some(latest.version) {
        return None;
    }
    let file_named = |name: &str| latest.files.iter().find(|file| file.name == name).cloned();
    Some(UpdateOffer {
        version: latest.version,
        notes: latest.notes.clone(),
        web_page: latest.web_page.clone(),
        program: file_named(&kind.release_file_name())?,
        checksums: file_named(CHECKSUMS_FILE_NAME)?,
    })
}

/// Busca en un `SHA256SUMS.txt` (formato de `sha256sum`: `<suma>  <fichero>`)
/// la suma de un fichero, en minúsculas.
pub fn checksum_for(checksums_file: &str, file_name: &str) -> Option<String> {
    checksums_file.lines().find_map(|line| {
        let (checksum, name) = line.trim().split_once(char::is_whitespace)?;
        // `sha256sum` marca con `*` los ficheros leídos en modo binario.
        let name = name.trim().trim_start_matches('*');
        let looks_like_sha256 = checksum.len() == 64 && checksum.chars().all(|character| character.is_ascii_hexdigit());
        (name == file_name && looks_like_sha256).then(|| checksum.to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    const WINDOWS_PORTABLE: BuildKind = BuildKind { operating_system: OperatingSystem::Windows, is_portable: true };
    const WINDOWS: BuildKind = BuildKind { operating_system: OperatingSystem::Windows, is_portable: false };
    const LINUX: BuildKind = BuildKind { operating_system: OperatingSystem::Linux, is_portable: false };

    fn release(tag: &str, file_names: &[&str]) -> Release {
        Release {
            version: version(tag),
            notes: "Novedades".into(),
            web_page: "https://github.com/manuelmsni/console/releases/tag/v1.2.0".into(),
            files: file_names
                .iter()
                .map(|name| ReleaseFile { name: name.to_string(), download_url: format!("https://example.com/{name}") })
                .collect(),
        }
    }

    #[test]
    fn versions_are_read_with_or_without_v() {
        assert_eq!(version("v1.2.3"), Version { major: 1, minor: 2, patch: 3 });
        assert_eq!(version("1.2.3"), Version { major: 1, minor: 2, patch: 3 });
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse("1.2.3.4"), None);
        assert_eq!(Version::parse("1.2.3-beta"), None);
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(version("1.10.0") > version("1.9.9"));
        assert!(version("2.0.0") > version("1.99.99"));
        assert_eq!(version("1.2.3").to_string(), "1.2.3");
    }

    #[test]
    fn each_kind_of_program_has_its_own_file() {
        assert_eq!(WINDOWS.release_file_name(), "console-windows-x64.exe");
        assert_eq!(WINDOWS_PORTABLE.release_file_name(), "console-windows-x64-portable.exe");
        assert_eq!(LINUX.release_file_name(), "console-linux-x64");
    }

    #[test]
    fn a_newer_release_with_this_program_is_offered() {
        let latest = release("v1.2.0", &["console-windows-x64.exe", "console-windows-x64-portable.exe", CHECKSUMS_FILE_NAME]);
        let offer = update_to_offer(version("1.1.0"), WINDOWS_PORTABLE, &latest, None).unwrap();
        assert_eq!(offer.version, version("1.2.0"));
        assert_eq!(offer.program.name, "console-windows-x64-portable.exe");
    }

    #[test]
    fn nothing_is_offered_when_up_to_date_or_skipped() {
        let latest = release("v1.2.0", &["console-windows-x64.exe", CHECKSUMS_FILE_NAME]);
        assert_eq!(update_to_offer(version("1.2.0"), WINDOWS, &latest, None), None);
        assert_eq!(update_to_offer(version("1.3.0"), WINDOWS, &latest, None), None);
        assert_eq!(update_to_offer(version("1.1.0"), WINDOWS, &latest, Some(version("1.2.0"))), None);
    }

    #[test]
    fn a_release_without_this_program_is_not_offered() {
        let only_windows = release("v1.2.0", &["console-windows-x64.exe", CHECKSUMS_FILE_NAME]);
        assert_eq!(update_to_offer(version("1.1.0"), LINUX, &only_windows, None), None);
        assert_eq!(update_to_offer(version("1.1.0"), WINDOWS_PORTABLE, &only_windows, None), None);
    }

    #[test]
    fn a_release_without_checksums_is_not_offered() {
        let without_checksums = release("v1.2.0", &["console-windows-x64.exe"]);
        assert_eq!(update_to_offer(version("1.1.0"), WINDOWS, &without_checksums, None), None);
    }

    #[test]
    fn the_checksum_of_each_file_is_found() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let checksums = format!("{a}  console-windows-x64.exe\n{b} *console-windows-x64-portable.exe\nbasura\n");
        assert_eq!(checksum_for(&checksums, "console-windows-x64.exe"), Some(a));
        assert_eq!(checksum_for(&checksums, "console-windows-x64-portable.exe"), Some("b".repeat(64)));
        assert_eq!(checksum_for(&checksums, "console-linux-x64"), None);
    }
}
