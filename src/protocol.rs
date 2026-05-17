use vigem_rust::controller::ds4::Ds4SpecialButton;
use vigem_rust::{Ds4Button, Ds4Dpad, Ds4Report, X360Button, X360Report};

use crate::config::RawDeadzone;

pub const PKT_STATE: u8 = 0x01;
pub const PKT_BATTERY: u8 = 0x02;

#[inline]
fn scale_axis(val: i16, invert: bool) -> u8 {
    let scaled = ((val as i32 + 32768) >> 8) as u8;
    if invert { !scaled } else { scaled }
}

pub fn get_empty_x360_report() -> X360Report {
    X360Report::default()
}

pub fn get_empty_ds4_report() -> Ds4Report {
    let mut report = Ds4Report::default();
    
    // Neutral stick positions are 128 for ds4
    report.thumb_lx = 128;
    report.thumb_ly = 128;
    report.thumb_rx = 128;
    report.thumb_ry = 128;
    report.set_dpad(Ds4Dpad::Neutral);

    report
}

pub fn parse_ds4_state(data: &[u8], deadzone: &RawDeadzone) -> Ds4Report {
    let mut gamepad = Ds4Report::default();
    let x360 = X360Button::from_bits_retain(u16::from_le_bytes([data[0], data[1]]));

    let mut ds4_btns = Ds4Button::empty();

    if x360.contains(X360Button::A) {
        ds4_btns |= Ds4Button::CROSS;
    }
    if x360.contains(X360Button::B) {
        ds4_btns |= Ds4Button::CIRCLE;
    }
    if x360.contains(X360Button::X) {
        ds4_btns |= Ds4Button::SQUARE;
    }
    if x360.contains(X360Button::Y) {
        ds4_btns |= Ds4Button::TRIANGLE;
    }
    if x360.contains(X360Button::LEFT_SHOULDER) {
        ds4_btns |= Ds4Button::SHOULDER_LEFT;
    }
    if x360.contains(X360Button::RIGHT_SHOULDER) {
        ds4_btns |= Ds4Button::SHOULDER_RIGHT;
    }
    if x360.contains(X360Button::LEFT_THUMB) {
        ds4_btns |= Ds4Button::THUMB_LEFT;
    }
    if x360.contains(X360Button::RIGHT_THUMB) {
        ds4_btns |= Ds4Button::THUMB_RIGHT;
    }
    if x360.contains(X360Button::START) {
        ds4_btns |= Ds4Button::OPTIONS;
    }
    if x360.contains(X360Button::BACK) {
        ds4_btns |= Ds4Button::SHARE;
    }

    gamepad.buttons = ds4_btns.bits();

    if x360.contains(X360Button::GUIDE) {
        gamepad.special = Ds4SpecialButton::PS.bits();
    }

    gamepad.set_dpad(
        match (
            x360.contains(X360Button::DPAD_UP),
            x360.contains(X360Button::DPAD_DOWN),
            x360.contains(X360Button::DPAD_LEFT),
            x360.contains(X360Button::DPAD_RIGHT),
        ) {
            (true, _, false, true) => Ds4Dpad::NorthEast,
            (true, _, true, false) => Ds4Dpad::NorthWest,
            (true, _, false, false) => Ds4Dpad::North,
            (false, true, false, true) => Ds4Dpad::SouthEast,
            (false, true, true, false) => Ds4Dpad::SouthWest,
            (false, true, false, false) => Ds4Dpad::South,
            (false, false, false, true) => Ds4Dpad::East,
            (false, false, true, false) => Ds4Dpad::West,
            _ => Ds4Dpad::Neutral,
        },
    );

    gamepad.trigger_l = data[2];
    gamepad.trigger_r = data[3];

    gamepad.thumb_lx = scale_axis(
        deadzone.apply(i16::from_le_bytes([data[4], data[5]])),
        false,
    );
    gamepad.thumb_ly = scale_axis(deadzone.apply(i16::from_le_bytes([data[6], data[7]])), true);
    gamepad.thumb_rx = scale_axis(
        deadzone.apply(i16::from_le_bytes([data[8], data[9]])),
        false,
    );
    gamepad.thumb_ry = scale_axis(
        deadzone.apply(i16::from_le_bytes([data[10], data[11]])),
        true,
    );

    gamepad
}

pub fn parse_x360_state(data: &[u8], deadzone: &RawDeadzone) -> X360Report {
    let read_axis = |offset: usize| -> i16 {
        deadzone.apply(i16::from_le_bytes([data[offset], data[offset + 1]]))
    };

    X360Report {
        buttons: X360Button::from_bits_retain(u16::from_le_bytes([data[0], data[1]])),
        left_trigger: data[2],
        right_trigger: data[3],
        thumb_lx: read_axis(4),
        thumb_ly: read_axis(6),
        thumb_rx: read_axis(8),
        thumb_ry: read_axis(10),
    }
}
