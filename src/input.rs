use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_A, VK_C, VK_CONTROL, VK_V,
};

/// Sends a key down event.
fn key_down(vk: VIRTUAL_KEY) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                ..Default::default()
            },
        },
    }
}

/// Sends a key up event.
fn key_up(vk: VIRTUAL_KEY) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: KEYEVENTF_KEYUP,
                ..Default::default()
            },
        },
    }
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
