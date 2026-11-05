//! Traducciones de todos los textos que ve el usuario.
//!
//! Cada idioma tiene su fichero en `locales/<código>.json`, con las mismas
//! claves que `en.json` (el idioma base). El script de compilación los mete
//! todos en el ejecutable, así que añadir un idioma es solo añadir su fichero.
//!
//! En el código, los textos se piden por clave: `tr("tabs.new_tab")`, o con
//! datos, `tr_args("tabs.unsaved", &[("count", &3)])` para un texto como
//! `"{count} unsaved tabs"`. Los tests de este módulo comprueban que todas las
//! claves usadas en el código existen en todos los idiomas.
//!
//! Es un módulo transversal, sin dependencias del resto de la aplicación: lo
//! puede usar cualquier capa, también el dominio para sus mensajes de error.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{OnceLock, RwLock};

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// El idioma en el que está escrita la aplicación y al que se recurre si
/// a otro le falta un texto.
pub const BASE_LANGUAGE: &str = "en";
/// Clave con el nombre de cada idioma escrito en ese idioma ("English", "Español").
const LANGUAGE_NAME_KEY: &str = "language.name";

struct Locale {
    code: String,
    texts: HashMap<String, String>,
}

static LOCALES: OnceLock<Vec<Locale>> = OnceLock::new();
static CURRENT_LANGUAGE: RwLock<String> = RwLock::new(String::new());

fn locales() -> &'static [Locale] {
    LOCALES.get_or_init(|| {
        EMBEDDED_LOCALES
            .iter()
            .map(|(code, json)| Locale {
                code: code.to_string(),
                texts: serde_json::from_str(json).unwrap_or_else(|error| panic!("locales/{code}.json no es válido: {error}")),
            })
            .collect()
    })
}

fn locale(code: &str) -> Option<&'static Locale> {
    locales().iter().find(|locale| locale.code == code)
}

/// Elige el idioma: el preferido en los ajustes si existe; si no (o si es
/// "automático"), el de Windows si hay traducción; si tampoco, el inglés.
pub fn set_language(preferred: Option<&str>, system_language: &str) {
    let chosen = [preferred, Some(system_language)]
        .into_iter()
        .flatten()
        .find(|code| locale(code).is_some())
        .unwrap_or(BASE_LANGUAGE);
    if let Ok(mut current) = CURRENT_LANGUAGE.write() {
        *current = chosen.to_string();
    }
}

pub fn current_language() -> String {
    CURRENT_LANGUAGE.read().map(|current| current.clone()).unwrap_or_default()
}

/// Código y nombre de cada idioma disponible, el base primero.
pub fn available_languages() -> Vec<(String, String)> {
    let mut languages: Vec<(String, String)> = locales()
        .iter()
        .map(|locale| (locale.code.clone(), locale.texts.get(LANGUAGE_NAME_KEY).cloned().unwrap_or_else(|| locale.code.clone())))
        .collect();
    languages.sort_by_key(|(code, name)| (code != BASE_LANGUAGE, name.clone()));
    languages
}

/// El texto de la clave en el idioma actual (o en inglés si le falta; o la
/// propia clave si tampoco está, para que se note).
pub fn tr(key: &str) -> String {
    let current = current_language();
    let text = [current.as_str(), BASE_LANGUAGE]
        .into_iter()
        .filter_map(locale)
        .find_map(|locale| locale.texts.get(key).cloned())
        .unwrap_or_else(|| key.to_string());
    text
}

/// Como `tr`, sustituyendo cada `{nombre}` del texto por su valor.
pub fn tr_args(key: &str, values: &[(&str, &dyn Display)]) -> String {
    values
        .iter()
        .fold(tr(key), |text, (name, value)| text.replace(&format!("{{{name}}}"), &value.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use super::*;

    /// Las claves que el código pide, buscando las llamadas a las funciones de
    /// traducción en todos los ficheros de `src`.
    fn keys_used_in_code() -> BTreeSet<String> {
        let source_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut keys = BTreeSet::new();
        for file in rust_files_in(&source_folder) {
            let code = fs::read_to_string(&file).unwrap();
            // Los comentarios no cuentan (la documentación pone ejemplos de claves).
            let without_comments: String =
                code.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
            keys.extend(keys_requested_in(&without_comments));
        }
        keys
    }

    fn rust_files_in(folder: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in fs::read_dir(folder).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                files.extend(rust_files_in(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
        files
    }

    /// La clave (texto entre comillas) que sigue a cada llamada a una función
    /// de traducción, aunque la llamada ocupe varias líneas.
    fn keys_requested_in(code: &str) -> Vec<String> {
        let function_names = [["tr", "("].concat(), ["tr_args", "("].concat()];
        let mut keys = Vec::new();
        for function_name in &function_names {
            for (position, _) in code.match_indices(function_name.as_str()) {
                let is_whole_word = code[..position].chars().last().map_or(true, |before| !before.is_alphanumeric() && before != '_');
                if !is_whole_word {
                    continue;
                }
                let after_call = code[position + function_name.len()..].trim_start();
                if let Some(rest) = after_call.strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        keys.push(rest[..end].to_string());
                    }
                }
            }
        }
        keys
    }

    fn placeholders_in(text: &str) -> BTreeSet<String> {
        text.split('{').skip(1).filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string())).collect()
    }

    fn keys_of(locale: &Locale) -> BTreeSet<String> {
        locale.texts.keys().cloned().collect()
    }

    #[test]
    fn there_is_an_english_file_and_at_least_one_more() {
        assert!(locale(BASE_LANGUAGE).is_some(), "falta locales/{BASE_LANGUAGE}.json");
        assert!(locales().len() >= 2);
    }

    #[test]
    fn every_key_used_in_the_code_exists_in_every_language() {
        let used = keys_used_in_code();
        assert!(used.len() > 50, "se esperaban muchas claves; ¿ha cambiado la forma de llamar a tr?");
        for locale in locales() {
            let missing: Vec<&String> = used.iter().filter(|key| !locale.texts.contains_key(*key)).collect();
            assert!(missing.is_empty(), "a locales/{}.json le faltan: {missing:#?}", locale.code);
        }
    }

    #[test]
    fn every_language_has_exactly_the_keys_of_english() {
        let english = keys_of(locale(BASE_LANGUAGE).unwrap());
        for locale in locales() {
            let keys = keys_of(locale);
            let missing: Vec<_> = english.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&english).collect();
            assert!(missing.is_empty(), "a locales/{}.json le faltan: {missing:#?}", locale.code);
            assert!(extra.is_empty(), "locales/{}.json tiene claves que no están en en.json: {extra:#?}", locale.code);
        }
    }

    #[test]
    fn every_english_key_is_used_in_the_code() {
        let used = keys_used_in_code();
        let unused: Vec<String> = keys_of(locale(BASE_LANGUAGE).unwrap())
            .into_iter()
            .filter(|key| key != LANGUAGE_NAME_KEY && !used.contains(key))
            .collect();
        assert!(unused.is_empty(), "claves de en.json que ya no usa el código: {unused:#?}");
    }

    #[test]
    fn translations_keep_the_same_placeholders() {
        let english = locale(BASE_LANGUAGE).unwrap();
        for locale in locales() {
            for (key, english_text) in &english.texts {
                let Some(text) = locale.texts.get(key) else { continue };
                assert_eq!(
                    placeholders_in(text),
                    placeholders_in(english_text),
                    "«{key}» en locales/{}.json no tiene los mismos {{marcadores}} que en inglés",
                    locale.code
                );
            }
        }
    }

    #[test]
    fn placeholders_are_filled_in() {
        set_language(Some(BASE_LANGUAGE), BASE_LANGUAGE);
        let text = tr_args("tabs.reopened_many", &[("count", &3)]);
        assert!(text.contains('3') && !text.contains("{count}"));
    }
}
