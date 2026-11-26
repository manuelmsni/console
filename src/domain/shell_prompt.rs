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

/// Si se está escribiendo en PowerShell: en su prompt (`PS C:\...> `) o en
/// una línea de continuación de un comando de varias líneas (`>> `).
pub fn is_powershell_input_line(line: &str) -> bool {
    let is_prompt = line.starts_with("PS ") && text_after_local_prompt(line).is_some();
    let is_continuation = line.starts_with(">> ") || line == ">>";
    is_prompt || is_continuation
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
    fn recognises_where_powershell_is_reading() {
        assert!(is_powershell_input_line(r"PS C:\Users\yo> Get-ChildItem"));
        assert!(is_powershell_input_line(">> 'segunda línea'"));
        assert!(!is_powershell_input_line(r"C:\Users\yo>dir"), "eso es cmd");
        assert!(!is_powershell_input_line(">>> print(1)"), "eso es Python");
        assert!(!is_powershell_input_line("> Escribe tu mensaje"));
    }

    #[test]
    fn other_lines_are_not_prompts() {
        assert_eq!(text_after_local_prompt("usuario@servidor:~ $ ls"), None);
        assert_eq!(text_after_local_prompt(">>> print(1)"), None);
        assert_eq!(text_after_local_prompt("> Escribe tu mensaje"), None);
    }
}
