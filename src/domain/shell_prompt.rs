//! Reconocer si una línea de la pantalla es el prompt de una consola local.

/// Si la línea es un prompt de cmd (`C:\Users\yo>dir`) o de PowerShell
/// (`PS C:\Users\yo> dir`), devuelve lo escrito después del prompt.
/// En cualquier otro caso (dentro de un programa, por SSH...) devuelve `None`.
pub fn text_after_local_prompt(line: &str) -> Option<&str> {
    let line = line.strip_prefix("PS ").unwrap_or(line);
    if !starts_with_drive_path(line) {
        return None;
    }
    // Las rutas de Windows no pueden contener '>', así que el primero cierra el prompt.
    let prompt_end = line.find('>')?;
    Some(line[prompt_end + 1..].trim())
}

fn starts_with_drive_path(line: &str) -> bool {
    let mut characters = line.chars();
    matches!(
        (characters.next(), characters.next(), characters.next()),
        (Some(drive_letter), Some(':'), Some('\\')) if drive_letter.is_ascii_alphabetic()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_cmd_prompt() {
        assert_eq!(text_after_local_prompt(r"C:\Users\yo>dir /b"), Some("dir /b"));
        assert_eq!(text_after_local_prompt(r"C:\>"), Some(""));
    }

    #[test]
    fn recognises_powershell_prompt() {
        assert_eq!(text_after_local_prompt(r"PS C:\Users\yo> Get-ChildItem"), Some("Get-ChildItem"));
    }

    #[test]
    fn redirections_stay_in_the_command() {
        assert_eq!(text_after_local_prompt(r"C:\tmp>dir > lista.txt"), Some("dir > lista.txt"));
    }

    #[test]
    fn other_lines_are_not_prompts() {
        assert_eq!(text_after_local_prompt("usuario@servidor:~ $ ls"), None);
        assert_eq!(text_after_local_prompt(">>> print(1)"), None);
        assert_eq!(text_after_local_prompt("> Escribe tu mensaje"), None);
    }
}
