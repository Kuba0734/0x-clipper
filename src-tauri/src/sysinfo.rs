//! Prozess-Metriken für die Statusanzeige und die Performance-Abnahme.

use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows::Win32::Foundation::FILETIME;

/// Private Bytes des eigenen Prozesses.
pub fn private_bytes() -> u64 {
    let mut c = PROCESS_MEMORY_COUNTERS_EX::default();
    // SAFETY: c ist korrekt dimensioniert; Pseudo-Handle des eigenen Prozesses.
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
    };
    if ok.is_ok() {
        c.PrivateUsage as u64
    } else {
        0
    }
}

/// CPU-Zeit (Kernel + User) des eigenen Prozesses in 100-ns-Einheiten.
pub fn cpu_time_100ns() -> u64 {
    let (mut a, mut b, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: Ausgabezeiger gültig.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u) }.is_err() {
        return 0;
    }
    let f = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    f(k) + f(u)
}
