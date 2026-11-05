use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

use crate::application::ports::Clock;
use crate::domain::local_time::LocalDateTime;

/// La hora local de Windows (con su zona horaria y horario de verano).
pub struct WindowsClock;

impl Clock for WindowsClock {
    fn now(&self) -> LocalDateTime {
        // Seguro: GetLocalTime solo rellena la estructura que le pasamos.
        let time = unsafe {
            let mut time: SYSTEMTIME = std::mem::zeroed();
            GetLocalTime(&mut time);
            time
        };
        LocalDateTime {
            year: time.wYear,
            month: time.wMonth as u8,
            day: time.wDay as u8,
            hour: time.wHour as u8,
            minute: time.wMinute as u8,
        }
    }
}
