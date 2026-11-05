//! Reglas de la contraseña de la versión portable, con la que se cifra todo
//! lo que guarda la aplicación.

pub const MINIMUM_PASSWORD_LENGTH: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum NewPasswordProblem {
    TooShort,
    /// La contraseña y su repetición no son iguales.
    DoesNotMatch,
}

/// Al crearla se pide dos veces: si se escribe mal y no se nota, los datos
/// quedarían cifrados con una contraseña que nadie sabe.
pub fn check_new_password(password: &str, repeated: &str) -> Result<(), NewPasswordProblem> {
    if password.chars().count() < MINIMUM_PASSWORD_LENGTH {
        return Err(NewPasswordProblem::TooShort);
    }
    if password != repeated {
        return Err(NewPasswordProblem::DoesNotMatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_enough_password_written_twice_is_fine() {
        assert_eq!(check_new_password("caballo grapa", "caballo grapa"), Ok(()));
    }

    #[test]
    fn a_short_password_is_rejected() {
        assert_eq!(check_new_password("corta", "corta"), Err(NewPasswordProblem::TooShort));
    }

    #[test]
    fn length_counts_letters_not_bytes() {
        // 8 letras, aunque con tildes ocupen más bytes.
        assert_eq!(check_new_password("áéíóúñüç", "áéíóúñüç"), Ok(()));
    }

    #[test]
    fn both_passwords_must_be_the_same() {
        assert_eq!(check_new_password("caballo grapa", "caballo grap"), Err(NewPasswordProblem::DoesNotMatch));
    }
}