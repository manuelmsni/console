//! Fecha y hora local, para dejar constancia de cuándo se abrió cada sesión.

use std::fmt;

use crate::i18n::tr_args;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

impl LocalDateTime {
    /// `05/11/2026 a las 00:12` (en el idioma de la interfaz).
    pub fn spoken(&self) -> String {
        let date = format!("{:02}/{:02}/{}", self.day, self.month, self.year);
        let time = format!("{:02}:{:02}", self.hour, self.minute);
        tr_args("time.date_at_time", &[("date", &date), ("time", &time)])
    }
}

impl fmt::Display for LocalDateTime {
    /// `05/11/2026 00:12`
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:02}/{:02}/{} {:02}:{:02}", self.day, self.month, self.year, self.hour, self.minute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_digits_get_a_leading_zero() {
        let moment = LocalDateTime { year: 2026, month: 11, day: 5, hour: 0, minute: 7 };
        assert_eq!(moment.to_string(), "05/11/2026 00:07");
        // La frase depende del idioma; la fecha y la hora, no.
        assert!(moment.spoken().contains("05/11/2026") && moment.spoken().contains("00:07"));
    }
}
