use super::{Operation, Target};
use std::os::windows::process::CommandExt;
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Registry::*, Threading::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const HIDE_PROCESS: u32 = 0x0800_0000;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn registry(root: HKEY, name: &str, value: &str) -> Option<String> {
    let mut key = std::ptr::null_mut();
    // SAFETY: nul-terminated input and bounded output; registry handles are closed on every path.
    unsafe {
        if RegOpenKeyExW(root, wide(name).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let mut buffer = [0u16; 4096];
        let mut size = std::mem::size_of_val(&buffer) as u32;
        let mut kind = 0;
        let ok = RegQueryValueExW(
            key,
            wide(value).as_ptr(),
            std::ptr::null(),
            &mut kind,
            buffer.as_mut_ptr().cast(),
            &mut size,
        );
        RegCloseKey(key);
        if ok != 0 || ![REG_SZ, REG_EXPAND_SZ].contains(&kind) || size < 2 {
            return None;
        }
        let count = (size as usize / 2).min(buffer.len());
        Some(
            String::from_utf16_lossy(&buffer[..count])
                .trim_end_matches('\0')
                .to_owned(),
        )
    }
}
fn browser() -> Result<PathBuf, String> {
    let prog = registry(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice", "ProgId")
        .ok_or("Cannot identify the Windows default browser; set a default HTTPS browser in Windows Settings")?;
    if prog.len() > 256 || prog.contains(['\\', '/']) {
        return Err("Invalid default-browser association".into());
    }
    let key = format!(r"{prog}\shell\open\command");
    let raw =
        registry(HKEY_CLASSES_ROOT, &key, "").ok_or("Default browser has no executable command")?;
    let executable = if let Some(rest) = raw.trim().strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        raw.split_whitespace().next().unwrap_or("")
    };
    let path = PathBuf::from(executable);
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "brave.exe",
        "zen.exe",
        "firefox.exe",
        "chrome.exe",
        "msedge.exe",
        "vivaldi.exe",
        "opera.exe",
    ]
    .contains(&filename.as_str())
    {
        return Err("Default browser is not supported for targeted controls; supported: Brave, Zen, Firefox, Chrome, Edge, Vivaldi, Opera".into());
    }
    existing(path)
}
fn existing(path: PathBuf) -> Result<PathBuf, String> {
    if !path.is_absolute() || !path.is_file() {
        return Err(format!(
            "Application executable is missing: {}",
            path.display()
        ));
    }
    path.canonicalize()
        .map_err(|e| format!("Cannot verify application path: {e}"))
}
fn configured(target: Target) -> Result<Option<PathBuf>, String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let roaming = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    let program = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    let path = match target {
        Target::Code => local.join(r"Programs\Microsoft VS Code\Code.exe"),
        Target::Telegram => roaming.join(r"Telegram Desktop\Telegram.exe"),
        Target::Telemost => program.join(r"Yandex\YandexTelemost\YandexTelemost.exe"),
        Target::Zotero => program.join(r"Zotero\zotero.exe"),
        Target::Browser => return browser().map(Some),
        Target::Codex => return Ok(None),
    };
    existing(path).map(Some)
}
fn process_path(window: HWND) -> Option<PathBuf> {
    // SAFETY: window handle comes from Windows; process handle is read-only and closed locally.
    unsafe {
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
        (ok != 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..length as usize])))
    }
}
struct Search {
    target: Target,
    path: Option<PathBuf>,
    found: HWND,
}
fn matches(search: &Search, path: &Path) -> bool {
    if let Some(expected) = &search.path {
        return path.canonicalize().is_ok_and(|actual| {
            actual
                .as_os_str()
                .eq_ignore_ascii_case(expected.as_os_str())
        });
    }
    if search.target != Target::Codex {
        return false;
    }
    let expected = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("WindowsApps");
    let Ok(relative) = path.strip_prefix(expected) else {
        return false;
    };
    let pieces: Vec<_> = relative.components().collect();
    pieces.len() == 3
        && pieces[0]
            .as_os_str()
            .to_string_lossy()
            .starts_with("OpenAI.Codex_")
        && pieces[0]
            .as_os_str()
            .to_string_lossy()
            .ends_with("_2p2nqsd0c76g0")
        && pieces[1].as_os_str().eq_ignore_ascii_case("app")
        && pieces[2].as_os_str().eq_ignore_ascii_case("ChatGPT.exe")
}
unsafe extern "system" fn visit(window: HWND, context: LPARAM) -> i32 {
    // SAFETY: EnumWindows receives the live stack Search pointer; callback never retains it.
    let search = unsafe { &mut *(context as *mut Search) };
    if unsafe { IsWindowVisible(window) } != 0
        && process_path(window).is_some_and(|path| matches(search, &path))
    {
        search.found = window;
        return 0;
    }
    1
}
fn find(search: &mut Search) -> HWND {
    // Preserve the owner's current window when the intended app is already foreground.
    let foreground = unsafe { GetForegroundWindow() };
    if process_path(foreground).is_some_and(|p| matches(search, &p)) {
        return foreground;
    }
    search.found = std::ptr::null_mut();
    unsafe {
        EnumWindows(Some(visit), search as *mut Search as LPARAM);
    }
    search.found
}
fn verified_foreground(window: HWND, search: &Search) -> bool {
    (unsafe { GetForegroundWindow() == window })
        && process_path(window).is_some_and(|p| matches(search, &p))
}
fn focus_diagnostic(window: HWND) -> String {
    if std::env::var_os("DECKWEAVE_WORKSPACE_TRACE").is_none() {
        return String::new();
    }
    fn describe(w: HWND) -> String {
        let mut class = [0u16; 128];
        let mut rect = RECT::default();
        // SAFETY: bounded buffers; diagnostics contain no window titles or document text.
        let (len, style, owner) = unsafe {
            (
                GetClassNameW(w, class.as_mut_ptr(), class.len() as i32),
                GetWindowLongPtrW(w, GWL_EXSTYLE),
                GetWindow(w, GW_OWNER),
            )
        };
        unsafe {
            GetWindowRect(w, &mut rect);
        }
        format!(
            "hwnd={:x},class={},exstyle={style:x},owner={:x},size={}x{}",
            w as usize,
            String::from_utf16_lossy(&class[..len.max(0) as usize]),
            owner as usize,
            rect.right - rect.left,
            rect.bottom - rect.top
        )
    }
    format!(
        " [target {}; foreground {}]",
        describe(window),
        describe(unsafe { GetForegroundWindow() })
    )
}
fn modifiers_down() -> bool {
    [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(*key as i32) } < 0)
}
fn activate_from_foreground(window: HWND, observed: HWND) {
    if observed.is_null() || modifiers_down() || unsafe { GetForegroundWindow() } != observed {
        return;
    }
    let current = unsafe { GetCurrentThreadId() };
    let foreground = unsafe { GetWindowThreadProcessId(observed, std::ptr::null_mut()) };
    if foreground == 0 || foreground == current {
        return;
    }
    // Create this helper thread's input queue, without consuming any input.
    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    // A deliberate hardware action may arrive through HID while another app owns
    // foreground input. Share its input queue briefly; never synthesize Alt/Win
    // keys or change the system foreground-lock setting to acquire activation.
    if unsafe { AttachThreadInput(current, foreground, 1) } == 0 {
        return;
    }
    struct Attached(u32, u32);
    impl Drop for Attached {
        fn drop(&mut self) {
            unsafe {
                AttachThreadInput(self.0, self.1, 0);
            }
        }
    }
    let _attached = Attached(current, foreground);
    if unsafe { GetForegroundWindow() } == observed {
        unsafe {
            BringWindowToTop(window);
            SetForegroundWindow(window);
        }
    }
    // Detach before checking the result and before injecting any application chord.
}
pub fn execute(op: Operation) -> Result<(), String> {
    let mut search = Search {
        target: op.target,
        path: configured(op.target)?,
        found: std::ptr::null_mut(),
    };
    let mut window = find(&mut search);
    if window.is_null() {
        let mut command = if let Some(path) = &search.path {
            Command::new(path)
        } else {
            let mut cmd = Command::new(
                PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()))
                    .join("explorer.exe"),
            );
            cmd.arg(r"shell:AppsFolder\OpenAI.Codex_2p2nqsd0c76g0!App");
            cmd
        };
        command
            .creation_flags(HIDE_PROCESS)
            .spawn()
            .map_err(|e| format!("Cannot open the selected application: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while window.is_null() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            window = find(&mut search);
        }
    }
    if window.is_null() {
        return Err(
            "Application did not expose a usable window; open it manually and retry".into(),
        );
    }
    let previous_foreground = unsafe { GetForegroundWindow() };
    unsafe {
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        SetForegroundWindow(window);
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    while !verified_foreground(window, &search) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !verified_foreground(window, &search) {
        activate_from_foreground(window, previous_foreground);
        let deadline = Instant::now() + Duration::from_millis(300);
        while !verified_foreground(window, &search) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    if !verified_foreground(window, &search) {
        return Err(format!(
            "Windows did not focus the selected application; focus it manually and retry{}",
            focus_diagnostic(window)
        ));
    }
    if op.keys.is_empty() {
        return Ok(());
    }
    // Do not combine the owner's held modifiers with our explicit chord.
    if modifiers_down() {
        return Err("Release keyboard modifiers before using this workspace command".into());
    }
    let mut inputs = vec![];
    for (&key, release) in op
        .keys
        .iter()
        .map(|k| (k, false))
        .chain(op.keys.iter().rev().map(|k| (k, true)))
    {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: if release { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    // Verify both exact window and executable immediately before the single SendInput batch.
    if !verified_foreground(window, &search) {
        return Err("Focus changed; press the command again in the intended application".into());
    }
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        // Release our chord after a partial injection, without ever retrying the action.
        let releases = &inputs[op.keys.len()..];
        unsafe {
            SendInput(
                releases.len() as u32,
                releases.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
        return Err(
            "Windows blocked keyboard input; use applications at the same privilege level".into(),
        );
    }
    Ok(())
}
