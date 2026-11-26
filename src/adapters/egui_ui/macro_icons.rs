//! Los iconos que se pueden poner a las macros y carpetas de la barra. En el
//! fichero se guarda el nombre (`rocket_launch`), no el dibujo: así, si un día
//! cambia el tipo de letra de los iconos, las macros guardadas siguen igual.

use egui_material_icons::icons::*;

/// Nombre guardado y dibujo, en el orden en que se ofrecen.
pub(super) const MACRO_ICONS: &[(&str, &str)] = &[
    ("folder", ICON_FOLDER),
    ("folder_open", ICON_FOLDER_OPEN),
    ("home", ICON_HOME),
    ("terminal", ICON_TERMINAL),
    ("code", ICON_CODE),
    ("rocket_launch", ICON_ROCKET_LAUNCH),
    ("bolt", ICON_BOLT),
    ("play_arrow", ICON_PLAY_ARROW),
    ("apps", ICON_APPS),
    ("extension", ICON_EXTENSION),
    ("public", ICON_PUBLIC),
    ("language", ICON_LANGUAGE),
    ("link", ICON_LINK),
    ("bookmark", ICON_BOOKMARK),
    ("star", ICON_STAR),
    ("favorite", ICON_FAVORITE),
    ("description", ICON_DESCRIPTION),
    ("edit_note", ICON_EDIT_NOTE),
    ("download", ICON_DOWNLOAD),
    ("backup", ICON_BACKUP),
    ("cloud", ICON_CLOUD),
    ("storage", ICON_STORAGE),
    ("database", ICON_DATABASE),
    ("dns", ICON_DNS),
    ("laptop", ICON_LAPTOP),
    ("desktop_windows", ICON_DESKTOP_WINDOWS),
    ("settings", ICON_SETTINGS),
    ("tune", ICON_TUNE),
    ("build", ICON_BUILD),
    ("bug_report", ICON_BUG_REPORT),
    ("science", ICON_SCIENCE),
    ("smart_toy", ICON_SMART_TOY),
    ("sports_esports", ICON_SPORTS_ESPORTS),
    ("music_note", ICON_MUSIC_NOTE),
    ("movie", ICON_MOVIE),
    ("photo", ICON_PHOTO),
    ("mail", ICON_MAIL),
    ("chat", ICON_CHAT),
    ("calendar_month", ICON_CALENDAR_MONTH),
    ("calculate", ICON_CALCULATE),
    ("work", ICON_WORK),
    ("school", ICON_SCHOOL),
    ("shopping_cart", ICON_SHOPPING_CART),
    ("key", ICON_KEY),
    ("lock", ICON_LOCK),
    ("wifi", ICON_WIFI),
    ("print", ICON_PRINT),
    ("restart_alt", ICON_RESTART_ALT),
    ("power_settings_new", ICON_POWER_SETTINGS_NEW),
];

/// El dibujo de un icono guardado. Un nombre desconocido (fichero editado a
/// mano, versión más nueva) se queda sin icono en lugar de fallar.
pub(super) fn icon_glyph(name: Option<&str>) -> Option<&'static str> {
    let name = name?;
    MACRO_ICONS.iter().find(|(known, _)| *known == name).map(|(_, glyph)| *glyph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_its_own_name() {
        let mut names: Vec<&str> = MACRO_ICONS.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), MACRO_ICONS.len());
    }

    #[test]
    fn unknown_icons_are_left_out() {
        assert_eq!(icon_glyph(Some("folder")), Some(ICON_FOLDER));
        assert_eq!(icon_glyph(Some("no_existe")), None);
        assert_eq!(icon_glyph(None), None);
    }
}
