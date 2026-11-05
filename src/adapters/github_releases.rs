//! Las releases publicadas en GitHub, para buscar versiones nuevas. Usa la API
//! pública sin identificarse (GitHub permite unas 60 consultas por hora y
//! equipo; la app hace una al abrir y otra al día como mucho).

use std::io::Read;
use std::time::Duration;

use serde::Deserialize;

use crate::application::ports::ReleaseSource;
use crate::domain::updates::{Release, ReleaseFile, Version};
use crate::i18n::tr_args;

const API_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// Un ejecutable de la app ocupa unos MB; esto es solo un tope de seguridad.
const MAXIMUM_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

pub struct GitHubReleases {
    /// `usuario/repositorio`.
    repository: String,
    agent: ureq::Agent,
}

impl GitHubReleases {
    pub fn of(repository: &str, current_version: Version) -> Self {
        // GitHub pide un User-Agent: el nombre de la app y su versión, nada del usuario.
        let agent = ureq::AgentBuilder::new().user_agent(&format!("console-updater/{current_version}")).build();
        Self { repository: repository.to_string(), agent }
    }
}

/// Lo que interesa de la respuesta de la API.
#[derive(Deserialize)]
struct PublishedRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<PublishedFile>,
}

#[derive(Deserialize)]
struct PublishedFile {
    name: String,
    browser_download_url: String,
}

impl ReleaseSource for GitHubReleases {
    fn latest_release(&self) -> Result<Option<Release>, String> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repository);
        let response = match self.agent.get(&url).set("Accept", "application/vnd.github+json").timeout(API_TIMEOUT).call() {
            Ok(response) => response,
            // Sin ninguna release publicada todavía, GitHub contesta 404.
            Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(error) => return Err(tr_args("updates.error.check", &[("error", &error)])),
        };
        let json = response.into_string().map_err(|error| tr_args("updates.error.check", &[("error", &error)]))?;
        let published: PublishedRelease =
            serde_json::from_str(&json).map_err(|error| tr_args("updates.error.check", &[("error", &error)]))?;

        // Una etiqueta que no es una versión (`v1.2.3`) no se tiene en cuenta.
        let Some(version) = Version::parse(&published.tag_name) else { return Ok(None) };
        Ok(Some(Release {
            version,
            notes: published.body.unwrap_or_default(),
            web_page: published.html_url,
            files: published
                .assets
                .into_iter()
                .map(|file| ReleaseFile { name: file.name, download_url: file.browser_download_url })
                .collect(),
        }))
    }

    fn download(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self
            .agent
            .get(url)
            .timeout(DOWNLOAD_TIMEOUT)
            .call()
            .map_err(|error| tr_args("updates.error.download", &[("error", &error)]))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAXIMUM_DOWNLOAD_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|error| tr_args("updates.error.download", &[("error", &error)]))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Va por internet, así que no se ejecuta con los demás tests:
    /// `cargo test -- --ignored github`.
    #[test]
    #[ignore]
    fn reads_the_latest_release_of_a_public_repository() {
        let releases = GitHubReleases::of("BurntSushi/ripgrep", Version::parse("0.0.1").unwrap());
        let latest = releases.latest_release().unwrap().expect("ripgrep tiene releases con versión x.y.z");
        assert!(latest.version > Version::parse("13.0.0").unwrap());
        assert!(latest.files.iter().any(|file| file.download_url.starts_with("https://")));
    }

    #[test]
    #[ignore]
    fn a_repository_without_releases_is_not_an_error() {
        let releases = GitHubReleases::of("manuelmsni/console", Version::parse("0.0.1").unwrap());
        assert!(releases.latest_release().is_ok());
    }
}
