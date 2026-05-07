use vigem_rust::controller::ds4::Ds4SpecialButton;
use vigem_rust::{Ds4Button, Ds4Dpad, Ds4Report, X360Button, X360Report};

use crate::config::RawDeadzone;

#[repr(u8)]
pub enum PacketType {
    State = 0x1,
    Battery = 0x2,
}

impl TryFrom<u8> for PacketType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == PacketType::State as u8 => Ok(PacketType::State),
            x if x == PacketType::Battery as u8 => Ok(PacketType::Battery),
            _ => Err(()),
        }
    }
}

#[inline]
fn scale_axis(val: i16, invert: bool) -> u8 {
    // Shift range to 0..65535
    let temp = val as i32 + 32768;
    // Scale to 0..255
    let scaled = (temp >> 8) as u8;

    if invert { 255 - scaled } else { scaled }
}

pub fn parse_ds4_state(data: &[u8], deadzone: &RawDeadzone) -> Ds4Report {
    let mut gamepad = Ds4Report::default();

    let buttons_raw = u16::from_le_bytes([data[0], data[1]]);
    let x360_buttons = X360Button::from_bits_retain(buttons_raw);

    let mut ds4_btns = Ds4Button::empty();

    if x360_buttons.contains(X360Button::A) {
        ds4_btns |= Ds4Button::CROSS;
    }
    if x360_buttons.contains(X360Button::B) {
        ds4_btns |= Ds4Button::CIRCLE;
    }
    if x360_buttons.contains(X360Button::X) {
        ds4_btns |= Ds4Button::SQUARE;
    }
    if x360_buttons.contains(X360Button::Y) {
        ds4_btns |= Ds4Button::TRIANGLE;
    }

    if x360_buttons.contains(X360Button::LEFT_SHOULDER) {
        ds4_btns |= Ds4Button::SHOULDER_LEFT;
    }
    if x360_buttons.contains(X360Button::RIGHT_SHOULDER) {
        ds4_btns |= Ds4Button::SHOULDER_RIGHT;
    }

    if x360_buttons.contains(X360Button::LEFT_THUMB) {
        ds4_btns |= Ds4Button::THUMB_LEFT;
    }
    if x360_buttons.contains(X360Button::RIGHT_THUMB) {
        ds4_btns |= Ds4Button::THUMB_RIGHT;
    }

    if x360_buttons.contains(X360Button::START) {
        ds4_btns |= Ds4Button::OPTIONS;
    }
    if x360_buttons.contains(X360Button::BACK) {
        ds4_btns |= Ds4Button::SHARE;
    }

    gamepad.buttons = ds4_btns.bits();

    let mut special_btns = Ds4SpecialButton::empty();
    if x360_buttons.contains(X360Button::GUIDE) {
        special_btns |= Ds4SpecialButton::PS;
    }
    gamepad.special = special_btns.bits();

    let dpad = if x360_buttons.contains(X360Button::DPAD_UP) {
        if x360_buttons.contains(X360Button::DPAD_RIGHT) {
            Ds4Dpad::NorthEast
        } else if x360_buttons.contains(X360Button::DPAD_LEFT) {
            Ds4Dpad::NorthWest
        } else {
            Ds4Dpad::North
        }
    } else if x360_buttons.contains(X360Button::DPAD_DOWN) {
        if x360_buttons.contains(X360Button::DPAD_RIGHT) {
            Ds4Dpad::SouthEast
        } else if x360_buttons.contains(X360Button::DPAD_LEFT) {
            Ds4Dpad::SouthWest
        } else {
            Ds4Dpad::South
        }
    } else if x360_buttons.contains(X360Button::DPAD_RIGHT) {
        Ds4Dpad::East
    } else if x360_buttons.contains(X360Button::DPAD_LEFT) {
        Ds4Dpad::West
    } else {
        Ds4Dpad::Neutral
    };

    gamepad.set_dpad(dpad);

    gamepad.trigger_l = data[2];
    gamepad.trigger_r = data[3];

    let raw_lx = deadzone.apply(i16::from_le_bytes([data[4], data[5]]));
    gamepad.thumb_lx = scale_axis(raw_lx, false);

    let raw_ly = deadzone.apply(i16::from_le_bytes([data[6], data[7]]));
    gamepad.thumb_ly = scale_axis(raw_ly, true); // Invert Y

    let raw_rx = deadzone.apply(i16::from_le_bytes([data[8], data[9]]));
    gamepad.thumb_rx = scale_axis(raw_rx, false);

    let raw_ry = deadzone.apply(i16::from_le_bytes([data[10], data[11]]));
    gamepad.thumb_ry = scale_axis(raw_ry, true); // Invert Y

    gamepad
}

pub fn parse_x360_state(data: &[u8], deadzone: &RawDeadzone) -> X360Report {
    let mut gamepad = X360Report::default();

    let buttons_raw = u16::from_le_bytes([data[0], data[1]]);
    gamepad.buttons = X360Button::from_bits_retain(buttons_raw);

    gamepad.left_trigger = data[2];
    gamepad.right_trigger = data[3];

    let raw_lx_val = i16::from_le_bytes([data[4], data[5]]);
    gamepad.thumb_lx = deadzone.apply(raw_lx_val);

    let raw_ly_val = i16::from_le_bytes([data[6], data[7]]);
    gamepad.thumb_ly = deadzone.apply(raw_ly_val);

    let raw_rx_val = i16::from_le_bytes([data[8], data[9]]);
    gamepad.thumb_rx = deadzone.apply(raw_rx_val);

    let raw_ry_val = i16::from_le_bytes([data[10], data[11]]);
    gamepad.thumb_ry = deadzone.apply(raw_ry_val);

    gamepad
}
