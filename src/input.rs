//! Keyboard input simulation via SendInput, modifier-aware.
//!
//! Wrappers like `send_copy` check the user's physical modifier state (tracked
//! in [`crate::hooks`] from real — non-injected — key events) and only
//! press/release modifiers the user is *not* already holding. This preserves
//! the user's Ctrl/Shift/Alt state across our operations, which is critical
//! for rapid-cycle hotkey usage (e.g. holding Ctrl and tapping Pause in
//! succession) — otherwise our own `Ctrl up` at the end of Ctrl+C/Ctrl+V
//! would release the user's modifier from the OS's point of view, causing
//! the next hotkey press to be misinterpreted.

use crate::hooks;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_HOME,
    VK_INSERT, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_SHIFT, VK_UP, VK_V,
};

/// Sends Shift+Home — selects from cursor to line start. Used by the
/// fallback word-conversion path when UIA isn't available.
pub fn send_select_to_line_start() {
    let shift_held = hooks::user_holds_shift();
    let ctrl_held = hooks::user_holds_ctrl();

    let mut inputs: Vec<INPUT> = Vec::with_capacity(6);
    // Ctrl+Shift+Home jumps to document start; we only want line start.
    if ctrl_held {
        inputs.push(key_up(VK_CONTROL));
    }
    if !shift_held {
        inputs.push(key_down(VK_SHIFT));
    }
    inputs.push(key_down(VK_HOME));
    inputs.push(key_up(VK_HOME));
    if !shift_held {
        inputs.push(key_up(VK_SHIFT));
    }
    if ctrl_held {
        inputs.push(key_down(VK_CONTROL));
    }
    dispatch(&inputs);
}

/// Sends Shift+Right × count. When the current selection anchor is on the
/// right (as after Shift+Home or Shift+Left), this shrinks the selection
/// from the left by `count` characters.
pub fn send_select_n_right(count: usize) {
    if count == 0 {
        return;
    }
    let ctrl_held = hooks::user_holds_ctrl();
    let shift_held = hooks::user_holds_shift();

    let mut inputs: Vec<INPUT> = Vec::with_capacity(count * 2 + 4);
    if ctrl_held {
        inputs.push(key_up(VK_CONTROL));
    }
    if !shift_held {
        inputs.push(key_down(VK_SHIFT));
    }
    for _ in 0..count {
        inputs.push(key_down(VK_RIGHT));
        inputs.push(key_up(VK_RIGHT));
    }
    if !shift_held {
        inputs.push(key_up(VK_SHIFT));
    }
    if ctrl_held {
        inputs.push(key_down(VK_CONTROL));
    }
    dispatch(&inputs);
}

/// Returns true if the given virtual key is an "extended key" in Win32 terms.
/// Extended keys (arrows, navigation) need the KEYEVENTF_EXTENDEDKEY flag
/// in SendInput, otherwise they get interpreted as numpad equivalents.
fn is_extended_vk(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_LEFT
            | VK_RIGHT
            | VK_UP
            | VK_DOWN
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_INSERT
            | VK_DELETE
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

fn key_down(vk: VIRTUAL_KEY) -> INPUT {
    build_input(vk, false)
}

fn key_up(vk: VIRTUAL_KEY) -> INPUT {
    build_input(vk, true)
}

fn dispatch(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Sends Ctrl+`key`. If the user is already holding Ctrl, we don't press/release
/// it ourselves — just send the key — so the user's Ctrl hold is preserved.
fn send_ctrl_key(key: VIRTUAL_KEY) {
    let ctrl_held = hooks::user_holds_ctrl();
    let mut inputs: Vec<INPUT> = Vec::with_capacity(4);
    if !ctrl_held {
        inputs.push(key_down(VK_CONTROL));
    }
    inputs.push(key_down(key));
    inputs.push(key_up(key));
    if !ctrl_held {
        inputs.push(key_up(VK_CONTROL));
    }
    dispatch(&inputs);
}

/// Simulates Ctrl+C.
pub fn send_copy() {
    send_ctrl_key(VK_C);
}

/// Simulates Ctrl+V.
pub fn send_paste() {
    send_ctrl_key(VK_V);
}

/// Selects N characters to the left of the cursor by sending Shift+Left × N.
/// Temporarily releases Ctrl if held — otherwise Shift+Left with Ctrl held
/// becomes Ctrl+Shift+Left (word-step), not what we want here.
pub fn send_select_n_left(count: usize) {
    if count == 0 {
        return;
    }
    let ctrl_held = hooks::user_holds_ctrl();
    let shift_held = hooks::user_holds_shift();

    let mut inputs: Vec<INPUT> = Vec::with_capacity(count * 2 + 4);
    if ctrl_held {
        inputs.push(key_up(VK_CONTROL));
    }
    if !shift_held {
        inputs.push(key_down(VK_SHIFT));
    }
    for _ in 0..count {
        inputs.push(key_down(VK_LEFT));
        inputs.push(key_up(VK_LEFT));
    }
    if !shift_held {
        inputs.push(key_up(VK_SHIFT));
    }
    if ctrl_held {
        // Re-press Ctrl so subsequent user events are interpreted with Ctrl.
        inputs.push(key_down(VK_CONTROL));
    }
    dispatch(&inputs);
}
