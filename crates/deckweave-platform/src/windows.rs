pub fn foreground() -> Option<String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };
    // SAFETY: OS-owned window handle; process is opened read-only. Output buffers have explicit lengths.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
        CloseHandle(process);
        if ok == 0 {
            None
        } else {
            Some(String::from_utf16_lossy(&buffer[..length as usize]))
        }
    }
}
pub struct ProcessTree {
    _job: std::os::windows::io::OwnedHandle,
}
pub fn contain_process(child: &std::process::Child) -> Result<ProcessTree, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    // SAFETY: the new non-inheritable job is uniquely owned. Child supplies a live owned process
    // handle. The initialized limit structure lives through SetInformationJobObject.
    unsafe {
        let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if raw.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let job = OwnedHandle::from_raw_handle(raw);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(raw, child.as_raw_handle()) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(ProcessTree { _job: job })
    }
}
pub fn session_locked() -> Option<bool> {
    use windows_sys::Win32::System::RemoteDesktop::*;
    // SAFETY: WTS allocates a length-reported buffer; read only a checked structure and free once.
    unsafe {
        let mut session = 0;
        if ProcessIdToSessionId(std::process::id(), &mut session) == 0 {
            return None;
        }
        let mut buffer = std::ptr::null_mut();
        let mut size = 0;
        if WTSQuerySessionInformationW(
            std::ptr::null_mut(),
            session,
            WTSSessionInfoEx,
            &mut buffer,
            &mut size,
        ) == 0
        {
            return None;
        }
        let result = if size as usize >= std::mem::size_of::<WTSINFOEXW>() {
            let info = &*buffer.cast::<WTSINFOEXW>();
            if info.Level == 1 {
                match info.Data.WTSInfoExLevel1.SessionFlags as u32 {
                    WTS_SESSIONSTATE_LOCK => Some(true),
                    WTS_SESSIONSTATE_UNLOCK => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        WTSFreeMemory(buffer.cast());
        result
    }
}
pub fn hotkey(keys: &[String]) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    fn code(key: &str) -> Option<u16> {
        Some(match key {
            "Ctrl" => VK_CONTROL,
            "Alt" => VK_MENU,
            "Shift" => VK_SHIFT,
            "Win" => VK_LWIN,
            "Enter" => VK_RETURN,
            "Space" => VK_SPACE,
            "Tab" => VK_TAB,
            "Esc" => VK_ESCAPE,
            "Left" => VK_LEFT,
            "Right" => VK_RIGHT,
            "Up" => VK_UP,
            "Down" => VK_DOWN,
            "VolumeUp" => VK_VOLUME_UP,
            "VolumeDown" => VK_VOLUME_DOWN,
            "VolumeMute" => VK_VOLUME_MUTE,
            "PlayPause" => VK_MEDIA_PLAY_PAUSE,
            "NextTrack" => VK_MEDIA_NEXT_TRACK,
            "PreviousTrack" => VK_MEDIA_PREV_TRACK,
            _ if key.len() == 1
                && key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) =>
            {
                key.as_bytes()[0] as u16
            }
            _ => {
                let number = key.strip_prefix('F')?.parse::<u8>().ok()?;
                if !(1..=24).contains(&number) {
                    return None;
                }
                VK_F1 + u16::from(number) - 1
            }
        })
    }
    let codes: Vec<_> = keys
        .iter()
        .map(|k| code(k).ok_or_else(|| format!("Unsupported key: {k}")))
        .collect::<Result<_, _>>()?;
    let make = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: (if up { KEYEVENTF_KEYUP } else { 0 })
                    | if [
                        VK_LEFT,
                        VK_RIGHT,
                        VK_UP,
                        VK_DOWN,
                        VK_VOLUME_UP,
                        VK_VOLUME_DOWN,
                        VK_VOLUME_MUTE,
                        VK_MEDIA_PLAY_PAUSE,
                        VK_MEDIA_NEXT_TRACK,
                        VK_MEDIA_PREV_TRACK,
                    ]
                    .contains(&vk)
                    {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        0
                    },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs: Vec<_> = codes
        .iter()
        .map(|&k| make(k, false))
        .chain(codes.iter().rev().map(|&k| make(k, true)))
        .collect();
    // SAFETY: INPUT arrays are fully initialized and remain alive during SendInput.
    let written = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if written != inputs.len() as u32 {
        let releases: Vec<_> = codes.iter().rev().map(|&k| make(k, true)).collect();
        unsafe {
            SendInput(
                releases.len() as u32,
                releases.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
        return Err("Windows did not accept the keyboard action".into());
    }
    Ok(())
}
