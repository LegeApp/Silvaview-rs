/// Windows privilege elevation utilities

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
#[cfg(windows)]
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Check if the current process is running with Administrator privileges
#[cfg(windows)]
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = HANDLE::default();

        // Get process token
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length: u32 = 0;

        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Request elevation by relaunching the process with "runas"
/// This will show the UAC prompt and restart the app with admin privileges
#[cfg(windows)]
pub fn request_elevation() -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    // Get current executable path
    let exe = std::env::current_exe()?;

    // Get command line args to pass through
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Use ShellExecuteW with "runas" verb to trigger UAC
    // We do this via `cmd /c start` to avoid blocking
    Command::new("cmd")
        .args(&["/C", "start", "", exe.to_str().unwrap()])
        .args(&args)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()?;

    // Exit current (non-elevated) process
    std::process::exit(0);
}

#[cfg(not(windows))]
pub fn request_elevation() -> std::io::Result<()> {
    Ok(())
}
