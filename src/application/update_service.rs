//! Actualizar la aplicación: buscar si hay una versión nueva para este tipo de
//! programa, descargarla comprobando que llega intacta y cambiar el ejecutable.

use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::ports::{ProgramInstaller, ReleaseSource};
use crate::domain::updates::{checksum_for, update_to_offer, BuildKind, UpdateOffer, Version};
use crate::i18n::{tr, tr_args};

/// Se comparte con los hilos que buscan y descargan en segundo plano.
#[derive(Clone)]
pub struct UpdateService {
    releases: Arc<dyn ReleaseSource>,
    installer: Arc<dyn ProgramInstaller>,
    current_version: Version,
    build_kind: BuildKind,
}

impl UpdateService {
    pub fn new(
        releases: Arc<dyn ReleaseSource>,
        installer: Arc<dyn ProgramInstaller>,
        current_version: Version,
        build_kind: BuildKind,
    ) -> Self {
        Self { releases, installer, current_version, build_kind }
    }

    pub fn current_version(&self) -> Version {
        self.current_version
    }

    /// La actualización que se puede ofrecer, si hay alguna. Tarda (va por
    /// internet): llamarla fuera del hilo de la ventana.
    pub fn find_update(&self, skipped: Option<Version>) -> Result<Option<UpdateOffer>, String> {
        let Some(latest) = self.releases.latest_release()? else { return Ok(None) };
        Ok(update_to_offer(self.current_version, self.build_kind, &latest, skipped))
    }

    /// Descarga el programa nuevo, comprueba que su suma SHA-256 es la
    /// publicada y lo pone en lugar del actual. También tarda.
    pub fn download_and_install(&self, offer: &UpdateOffer) -> Result<(), String> {
        let checksums = self.releases.download(&offer.checksums.download_url)?;
        let checksums = String::from_utf8_lossy(&checksums);
        let expected = checksum_for(&checksums, &offer.program.name)
            .ok_or_else(|| tr_args("updates.error.no_checksum", &[("file", &offer.program.name)]))?;

        let new_program = self.releases.download(&offer.program.download_url)?;
        if sha256_hex(&new_program) != expected {
            return Err(tr("updates.error.checksum_mismatch"));
        }
        self.installer.replace_running_program(&new_program)
    }

    pub fn start_new_program(&self) -> Result<(), String> {
        self.installer.start_new_program()
    }

    pub fn clean_up_previous_program(&self) {
        self.installer.clean_up_previous_program();
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;
    use crate::domain::updates::{OperatingSystem, Release, ReleaseFile, CHECKSUMS_FILE_NAME};

    const KIND: BuildKind = BuildKind { operating_system: OperatingSystem::Windows, is_portable: false };

    struct FakeReleases {
        latest: Option<Release>,
        downloads: HashMap<String, Vec<u8>>,
    }

    impl ReleaseSource for FakeReleases {
        fn latest_release(&self) -> Result<Option<Release>, String> {
            Ok(self.latest.clone())
        }
        fn download(&self, url: &str) -> Result<Vec<u8>, String> {
            self.downloads.get(url).cloned().ok_or_else(|| format!("no existe {url}"))
        }
    }

    #[derive(Default)]
    struct FakeInstaller {
        installed: Mutex<Option<Vec<u8>>>,
    }

    impl ProgramInstaller for FakeInstaller {
        fn replace_running_program(&self, new_program: &[u8]) -> Result<(), String> {
            *self.installed.lock().unwrap() = Some(new_program.to_vec());
            Ok(())
        }
        fn start_new_program(&self) -> Result<(), String> {
            Ok(())
        }
        fn clean_up_previous_program(&self) {}
    }

    /// Una release 1.1.0 con el programa de Windows y sus sumas.
    fn service_with_release(program: &[u8], published_checksum: &str) -> (UpdateService, Arc<FakeInstaller>) {
        let file = |name: &str| ReleaseFile { name: name.to_string(), download_url: format!("https://example.com/{name}") };
        let latest = Release {
            version: Version::parse("1.1.0").unwrap(),
            notes: String::new(),
            web_page: String::new(),
            files: vec![file("console-windows-x64.exe"), file(CHECKSUMS_FILE_NAME)],
        };
        let checksums = format!("{published_checksum}  console-windows-x64.exe\n");
        let downloads = HashMap::from([
            ("https://example.com/console-windows-x64.exe".to_string(), program.to_vec()),
            (format!("https://example.com/{CHECKSUMS_FILE_NAME}"), checksums.into_bytes()),
        ]);
        let installer = Arc::new(FakeInstaller::default());
        let service = UpdateService::new(
            Arc::new(FakeReleases { latest: Some(latest), downloads }),
            installer.clone(),
            Version::parse("1.0.0").unwrap(),
            KIND,
        );
        (service, installer)
    }

    #[test]
    fn a_program_that_arrives_intact_is_installed() {
        let program = b"programa nuevo";
        let (service, installer) = service_with_release(program, &sha256_hex(program));

        let offer = service.find_update(None).unwrap().expect("hay una versión nueva");
        service.download_and_install(&offer).unwrap();

        assert_eq!(installer.installed.lock().unwrap().as_deref(), Some(&program[..]));
    }

    #[test]
    fn a_program_with_another_checksum_is_not_installed() {
        let (service, installer) = service_with_release(b"programa alterado", &sha256_hex(b"programa nuevo"));

        let offer = service.find_update(None).unwrap().unwrap();

        assert!(service.download_and_install(&offer).is_err());
        assert!(installer.installed.lock().unwrap().is_none());
    }

    #[test]
    fn sha256_matches_the_known_value() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}