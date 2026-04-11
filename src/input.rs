use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_A, VK_C, VK_CONTROL, VK_DELETE,
    VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_SHIFT,
    VK_UP, VK_V,
};

/// Returns true if the given virtual key is an "extended key" in Win32 terms.
/// Extended keys (arrows, navigation, numpad slash/enter, right Ctrl/Alt) need
/// the KEYEVENTF_EXTENDEDKEY flag in SendInput, otherwise they get interpreted
/// as numpad equivalents and many apps treat them differently.
fn is_extended_vk(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN
            | VK_HOME | VK_END | VK_PRIOR | VK_NEXT
            | VK_INSERT | VK_DELETE
    )
}

fn build_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended_vk(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

/// Sends a key down event.
fn key_down(vk: VIRTUAL_KEY) -> INPUT {
    build_input(vk, false)
}

/// Sends a key up event.
fn key_up(vk: VIRTUAL_KEY) -> INPUT {
    build_input(vk, true)
}

/// Simulates a Ctrl+Key press (e.g., Ctrl+C, Ctrl+V, Ctrl+A).
pub fn send_ctrl_key(key: VIRTUAL_KEY) {
    let inputs = [
        key_down(VK_CONTROL),
        key_down(key),
        key_up(key),
        key_up(VK_CONTROL),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Simulates Ctrl+C.
pub fn send_copy() {
    send_ctrl_key(VK_C);
}

/// Simulates Ctrl+V.
pub fn send_paste() {
    send_ctrl_key(VK_V);
}

/// Simulates Ctrl+A (select all).
pub fn send_select_all() {
    send_ctrl_key(VK_A);
}

/// Simulates a single key press and release.
pub fn send_key(key: VIRTUAL_KEY) {
    let inputs = [key_down(key), key_up(key)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Simulates Ctrl+Shift+Left — selects the word to the left of the cursor.
pub fn send_select_word_left() {
    let inputs = [
        key_down(VK_CONTROL),
        key_down(VK_SHIFT),
        key_down(VK_LEFT),
        key_up(VK_LEFT),
        key_up(VK_SHIFT),
        key_up(VK_CONTROL),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Selects N characters to the left of the cursor by sending Shift+Left N times.
/// Used to re-select text just pasted via Ctrl+V, so the user can press the
/// conversion hotkey again to cycle through layouts without manual reselection.
pub fn send_select_n_left(count: usize) {
    if count == 0 {
        return;
    }
    let mut inputs: Vec<INPUT> = Vec::with_capacity(2 + count * 2);
    inputs.push(key_down(VK_SHIFT));
    for _ in 0..count {
        inputs.push(key_down(VK_LEFT));
        inputs.push(key_up(VK_LEFT));
    }
    inputs.push(key_up(VK_SHIFT));
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Ensures Ctrl/Shift/Alt are released before we start sending injected keys.
/// First waits up to ~200ms for the user to release naturally; if they keep
/// holding, force-releases via SendInput.
pub fn wait_for_modifiers_release() {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(200);

    // Phase 1: wait for natural release
    loop {
        let (ctrl, shift, alt) = read_modifier_state();
        if !ctrl && !shift && !alt {
            std::thread::sleep(std::time::Duration::from_millis(20));
            return;
        }
        if start.elapsed() > timeout {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Phase 2: force release whatever is still held
    let (ctrl, shift, alt) = read_modifier_state();
    let mut inputs: Vec<INPUT> = Vec::new();
    if ctrl { inputs.push(key_up(VK_CONTROL)); }
    if shift { inputs.push(key_up(VK_SHIFT)); }
    if alt { inputs.push(key_up(VK_MENU)); }

    if !inputs.is_empty() {
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn read_modifier_state() -> (bool, bool, bool) {
    unsafe {
        (
            (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0,
            (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
            (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
        )
    }
}
