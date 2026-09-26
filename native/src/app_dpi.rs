//! Thread-local DPI scope. Never changes an application's process-wide awareness.
use windows::Win32::{Foundation::HWND, UI::HiDpi::*};

pub struct Scope(DPI_AWARENESS_CONTEXT);
impl Drop for Scope {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
fn contexts() -> [(i32, DPI_AWARENESS_CONTEXT); 5] {
    [
        (-1, DPI_AWARENESS_CONTEXT_UNAWARE),
        (-2, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE),
        (-3, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE),
        (-4, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2),
        (-5, DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED),
    ]
}
pub fn enter(kind: i32) -> Option<Scope> {
    let context = contexts().into_iter().find(|(k, _)| *k == kind)?.1;
    let previous = unsafe { SetThreadDpiAwarenessContext(context) };
    (!previous.0.is_null()).then_some(Scope(previous))
}
pub fn for_window(hwnd: HWND) -> Option<(Scope, i32)> {
    let actual = unsafe { GetWindowDpiAwarenessContext(hwnd) };
    let kind = contexts()
        .into_iter()
        .find(|(_, c)| unsafe { AreDpiAwarenessContextsEqual(actual, *c).as_bool() })?
        .0;
    Some((enter(kind)?, kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinate_scope_restores_the_calling_thread_and_refuses_unknown_contexts() {
        unsafe {
            let before = GetThreadDpiAwarenessContext();
            assert!(enter(12345).is_none());
            {
                let _scope = enter(-1).unwrap();
                assert!(AreDpiAwarenessContextsEqual(
                    GetThreadDpiAwarenessContext(),
                    DPI_AWARENESS_CONTEXT_UNAWARE
                )
                .as_bool());
            }
            assert!(AreDpiAwarenessContextsEqual(before, GetThreadDpiAwarenessContext()).as_bool());
        }
    }
}
