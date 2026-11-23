use std::cell::{OnceCell, RefCell};
use std::path::Path;
use std::time::{Duration, Instant};

use uiautomation::types::{ControlType, Handle, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{UIAutomation, UIElement};

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};
use zman_core::WindowInfo;
use zman_core::event::url_from_address_bar;

use super::Platform;

/// Browsers whose address bar zman knows how to read.
const BROWSERS: &[&str] = &[
    "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe", "arc.exe", "zen.exe",
];

/// How long to wait before looking again for an address bar that was not found.
const SEARCH_AGAIN: Duration = Duration::from_secs(30);

/// The address bar of the browser window last seen in the foreground.
struct AddressBar {
    hwnd: isize,
    /// `None` when the window has no address bar, or is a private window.
    element: Option<UIElement>,
    searched_at: Instant,
    url: Option<String>,
    /// Title the window had when `url` was read.
    title: String,
}

pub struct WindowsPlatform {
    read_address_bar: bool,
    /// UI Automation, started the first time a browser is in the foreground.
    automation: OnceCell<Option<UIAutomation>>,
    bar: RefCell<Option<AddressBar>>,
}

impl WindowsPlatform {
    pub fn new(read_address_bar: bool) -> WindowsPlatform {
        WindowsPlatform { read_address_bar, automation: OnceCell::new(), bar: RefCell::new(None) }
    }

    /// The page a browser window is showing, read from its address bar
    /// through the accessibility interface (what screen readers use).
    /// Only the address is read, never the content of the page.
    fn browser_url(&self, hwnd: HWND, title: &str) -> Option<String> {
        let automation = self.automation.get_or_init(|| UIAutomation::new().ok()).as_ref()?;
        let id = hwnd as isize;
        let mut slot = self.bar.borrow_mut();

        let stale = match slot.as_ref() {
            Some(bar) => bar.hwnd != id || (bar.element.is_none() && bar.searched_at.elapsed() > SEARCH_AGAIN),
            None => true,
        };
        if stale {
            *slot = Some(AddressBar { hwnd: id, element: find_address_bar(automation, id), searched_at: Instant::now(), url: None, title: String::new() });
        }

        let bar = slot.as_mut()?;
        let element = bar.element.as_ref()?;
        // While the user is typing in the bar, its text is not the page's
        // address: keep the last one, as long as it is still the same page.
        if element.has_keyboard_focus().unwrap_or(false) {
            return if bar.title == title { bar.url.clone() } else { None };
        }
        match element.get_property_value(UIProperty::ValueValue) {
            Ok(value) => {
                bar.url = value.get_string().ok().and_then(|text| url_from_address_bar(&text));
                bar.title = title.to_string();
                bar.url.clone()
            }
            // The window changed under us; look again on the next sample.
            Err(_) => {
                *slot = None;
                None
            }
        }
    }
}

/// Finds the address bar: the first text field of the window, which in every
/// supported browser comes before the page itself.
///
/// Private windows are never read. Browsers tell screen readers when a
/// window is private, in its name (Edge, Firefox) or in the profile button
/// of the toolbar (Chrome, Brave), and that is what is checked here.
fn find_address_bar(automation: &UIAutomation, hwnd: isize) -> Option<UIElement> {
    let window = automation.element_from_handle(Handle::from(hwnd)).ok()?;
    if is_private_label(&window.get_name().unwrap_or_default()) {
        return None;
    }
    let is_edit = automation
        .create_property_condition(UIProperty::ControlType, Variant::from(ControlType::Edit as i32), None)
        .ok()?;
    let bar = window.find_first(TreeScope::Descendants, &is_edit).ok()?;

    // Chromium's profile button reads "Incognito" in a private window.
    let is_profile_button = automation
        .create_property_condition(UIProperty::ClassName, Variant::from("AvatarToolbarButton"), None)
        .ok()?;
    if let Ok(button) = window.find_first(TreeScope::Descendants, &is_profile_button) {
        if is_private_label(&button.get_name().unwrap_or_default()) {
            return None;
        }
    }
    Some(bar)
}

/// Whether a window or profile name marks private browsing, in the
/// languages zman knows about.
fn is_private_label(name: &str) -> bool {
    let name = name.to_lowercase();
    ["incognit", "incógnit", "inkognito", "inprivate", "private", "privad", "privé", "privat"]
        .iter()
        .any(|mark| name.contains(mark))
}

impl Platform for WindowsPlatform {
    fn active_window(&self) -> Option<WindowInfo> {
        // SAFETY: no arguments; returns null when no window has focus.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return None;
        }
        // Elevated or protected processes may refuse to be queried.
        let app = process_name(hwnd).unwrap_or_else(|| "unknown".to_string());
        let is_browser = BROWSERS.iter().any(|browser| app.eq_ignore_ascii_case(browser));
        let title = window_title(hwnd);
        let url = if self.read_address_bar && is_browser { self.browser_url(hwnd, &title) } else { None };
        Some(WindowInfo { app, title, url })
    }

    fn idle_time(&self) -> Duration {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: `info` is a valid LASTINPUTINFO with cbSize set.
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return Duration::ZERO;
        }
        // Both are 32-bit tick counts that wrap every ~49 days.
        // SAFETY: no arguments.
        let now = unsafe { GetTickCount() };
        Duration::from_millis(now.wrapping_sub(info.dwTime) as u64)
    }
}

fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    // SAFETY: the buffer is valid for `buf.len()` UTF-16 units.
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// File name of the executable that owns the window, e.g. `chrome.exe`.
fn process_name(hwnd: HWND) -> Option<String> {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 {
        return None;
    }
    // SAFETY: plain call; a null handle means failure.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is open, the buffer is valid for `len` units, and
    // the handle is closed exactly once right after.
    let ok = unsafe {
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        ok
    };
    if ok == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let name = Path::new(&path).file_name()?.to_string_lossy().into_owned();
    Some(name)
}
