use rmk::types::action::{Action, EncoderAction, KeyAction};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{MorseMode, MorseProfile};
use rmk::{a, encoder, k, kbctrl, ltp, shifted, tg, user, wm};

use crate::nb::{mac, win};

/// Unified 5x14 matrix: left half is cols 0-5, right half cols 7-13. Col 6 is
/// unused, as in the old ZMK matrix transform.
pub(crate) const ROW: usize = 5;
pub(crate) const COL: usize = 14;
pub(crate) const NUM_LAYER: usize = 7;
pub(crate) const NUM_ENCODER: usize = 1;

/// Layers that turn on ADJUST when both are held (WIN: SYMBOL + NAV).
pub(crate) const TRI_LAYER: [u8; 3] = [1, 2, 6];

/// Morse profile index of the thumb layer-taps (`my_mt` in ZMK)
pub(crate) const HT: u8 = 0;
/// `my_mt`: hold-preferred, tapping-term 150ms, quick-tap 150ms
pub(crate) const HOLD_TAP_PROFILE: MorseProfile =
    MorseProfile::new(None, Some(MorseMode::HoldOnOtherPress), Some(150), None).with_quick_tap_timeout_ms(Some(150));

const _______: KeyAction = a!(Transparent);
const XXXXXXX: KeyAction = a!(No);

const fn mods(m: ModifierCombination) -> KeyAction {
    KeyAction::Single(Action::Modifier(m))
}

const CTRL: ModifierCombination = ModifierCombination::LCTRL;
const ALT: ModifierCombination = ModifierCombination::LALT;
const GUI: ModifierCombination = ModifierCombination::LGUI;
const GUI_ALT: KeyAction = mods(GUI.with_left_alt(true));
const GUI_SHIFT_ALT: KeyAction = mods(GUI.with_left_shift(true).with_left_alt(true));

// BLE keys, with 5 profiles (see keyboard.toml):
// User0-4 select a profile (hold 5s to clear it), User7 clears the current one.
const BT_CLR: KeyAction = user!(7);
/// Switch to the dongle bond slot; hold 5s to forget the dongle and re-pair.
const DONGLE: KeyAction = user!(10);

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        // 0: WIN
        [
            [k!(Escape),    k!(Kc1),    k!(Kc2),    k!(Kc3),    k!(Kc4),               k!(Kc5),    XXXXXXX,    k!(Up),     k!(Kc6),    k!(Kc7),                  k!(Kc8),    k!(Kc9),    k!(Kc0),      win::PLUS],
            [k!(Tab),       k!(Q),      k!(W),      k!(E),      k!(R),                 k!(T),      XXXXXXX,    k!(Down),   k!(Y),      k!(U),                    k!(I),      k!(O),      k!(P),        win::A_RING],
            [k!(LGui),      k!(A),      k!(S),      k!(D),      k!(F),                 k!(G),      XXXXXXX,    k!(Left),   k!(H),      k!(J),                    k!(K),      k!(L),      win::O_SLASH, win::AE],
            [k!(LShift),    k!(Z),      k!(X),      k!(C),      k!(V),                 k!(B),      XXXXXXX,    k!(Right),  k!(N),      k!(M),                    k!(Comma),  k!(Dot),    win::MINUS,   win::APOS],
            [k!(AudioMute), k!(LGui),   k!(LAlt),   k!(LCtrl),  ltp!(1, Delete, HT),   k!(Enter),  XXXXXXX,    k!(Enter),  k!(Space),  ltp!(2, Backspace, HT),   k!(Delete), k!(RAlt),   k!(RGui),     XXXXXXX],
        ],
        // 1: SYMBOL
        [
            [win::ACUTE,    k!(F1),     k!(F2),     k!(F3),     k!(F4),                k!(F5),     XXXXXXX,    k!(MouseUp),    k!(F6),     k!(F7),     k!(F8),     k!(F9),     k!(F10),    k!(F11)],
            [XXXXXXX,       k!(Kc1),    k!(Kc2),    k!(Kc3),    k!(Kc4),               k!(Kc5),    XXXXXXX,    k!(MouseDown),  win::TILDE, k!(Kc7),    k!(Kc8),    k!(Kc9),    k!(Kc0),    win::TILDE],
            [XXXXXXX,       win::EXCL,  win::AT,    win::HASH,  win::LT,               win::GT,    XXXXXXX,    k!(MouseLeft),  win::LPAR,  win::RPAR,  win::ASTRK, win::CARET, win::PIPE,  win::APOS],
            [XXXXXXX,       win::EQUAL, win::MINUS, win::PLUS,  win::LBRC,             win::RBRC,  XXXXXXX,    k!(MouseRight), win::LBKT,  win::RBKT,  win::SEMI,  win::COLON, win::EQUAL, win::BSLH],
            [_______,       _______,    _______,    _______,    _______,               _______,    XXXXXXX,    k!(MouseBtn1),  _______,    _______,    _______,    _______,    _______,    XXXXXXX],
        ],
        // 2: NAV
        [
            [k!(Grave),     k!(F1),     k!(F2),     k!(F3),     k!(F4),     k!(F5),     XXXXXXX,    _______,    k!(F6),       k!(F7),             k!(F8),     k!(F9),              k!(F10),                 _______],
            [_______,       k!(Insert), k!(PrintScreen), k!(Menu), _______, _______,    XXXXXXX,    _______,    k!(PageUp),   wm!(Left, CTRL),    k!(Up),     wm!(Right, CTRL),    wm!(Backspace, CTRL),    k!(Backspace)],
            [_______,       k!(LAlt),   k!(LCtrl),  k!(LShift), _______,    kbctrl!(CapsWordToggle), XXXXXXX, _______, k!(PageDown), k!(Left),   k!(Down),   k!(Right),           k!(Delete),              k!(Backspace)],
            [_______,       k!(Undo),   k!(Cut),    k!(Copy),   k!(Paste),  _______,    XXXXXXX,    _______,    XXXXXXX,      k!(Home),           XXXXXXX,    k!(End),             XXXXXXX,                 _______],
            [k!(AudioMute), _______,    _______,    _______,    _______,    _______,    XXXXXXX,    _______,    _______,      _______,            _______,    _______,             _______,                 XXXXXXX],
        ],
        // 3: MAC (toggled from ADJUST, sits on top of WIN)
        [
            [k!(Escape),    k!(Kc1),    k!(Kc2),    k!(Kc3),    k!(Kc4),               k!(Kc5),    XXXXXXX,    k!(Up),     k!(Kc6),    k!(Kc7),                  k!(Kc8),    k!(Kc9),    k!(Kc0),      mac::PLUS],
            [k!(Tab),       k!(Q),      k!(W),      k!(E),      k!(R),                 k!(T),      XXXXXXX,    k!(Down),   k!(Y),      k!(U),                    k!(I),      k!(O),      k!(P),        mac::A_RING],
            [k!(LCtrl),     k!(A),      k!(S),      k!(D),      k!(F),                 k!(G),      XXXXXXX,    k!(Left),   k!(H),      k!(J),                    k!(K),      k!(L),      mac::O_SLASH, mac::AE],
            [k!(LShift),    k!(Z),      k!(X),      k!(C),      k!(V),                 k!(B),      XXXXXXX,    k!(Right),  k!(N),      k!(M),                    k!(Comma),  k!(Dot),    mac::MINUS,   mac::APOS],
            [k!(AudioMute), k!(LCtrl),  k!(LAlt),   k!(LGui),   ltp!(4, Delete, HT),   k!(Enter),  XXXXXXX,    k!(Enter),  k!(Space),  ltp!(5, Backspace, HT),   GUI_ALT,    k!(RAlt),   GUI_SHIFT_ALT, XXXXXXX],
        ],
        // 4: SYM_MAC
        [
            [mac::ACUTE,    k!(F1),     k!(F2),     k!(F3),     k!(F4),     k!(F5),     XXXXXXX,    k!(MouseUp),    k!(F6),     k!(F7),                  k!(F8),     k!(F9),          k!(F10),    k!(F11)],
            [XXXXXXX,       k!(Kc1),    k!(Kc2),    k!(Kc3),    k!(Kc4),    k!(Kc5),    XXXXXXX,    k!(MouseDown),  mac::TILDE, k!(Kc7),                 k!(Kc8),    k!(Kc9),         k!(Kc0),    mac::TILDE],
            [XXXXXXX,       mac::EXCL,  mac::AT,    mac::HASH,  mac::LT,    mac::GT,    XXXXXXX,    k!(MouseLeft),  mac::LPAR,  mac::RPAR,               mac::ASTRK, mac::CIRCUMFLEX, mac::PIPE,  mac::APOS],
            [XXXXXXX,       mac::EQUAL, mac::MINUS, mac::PLUS,  mac::LBRC,  mac::RBRC,  XXXXXXX,    k!(MouseRight), mac::LBKT,  mac::RBKT,               mac::SEMI,  mac::COLON,      mac::EQUAL, mac::BSLH],
            // RMK has only one tri-layer, so the other thumb reaches ADJUST
            // directly here (ZMK: conditional layer 4 + 5 -> 6)
            [_______,       _______,    _______,    _______,    _______,    _______,    XXXXXXX,    k!(MouseBtn1),  _______,    ltp!(6, Backspace, HT),  _______,    _______,         _______,    XXXXXXX],
        ],
        // 5: NAV_MAC
        [
            [k!(Grave),     k!(F1),     k!(F2),     k!(F3),     k!(F4),     k!(F5),     XXXXXXX,    _______,    k!(F6),       k!(F7),             k!(F8),     k!(F9),              k!(F10),                 _______],
            [_______,       k!(Insert), k!(PrintScreen), k!(Menu), _______, _______,    XXXXXXX,    _______,    k!(PageUp),   wm!(Left, ALT),     k!(Up),     wm!(Right, ALT),     wm!(Backspace, CTRL),    k!(Backspace)],
            [_______,       k!(LAlt),   k!(LCtrl),  k!(LShift), _______,    kbctrl!(CapsWordToggle), XXXXXXX, _______, k!(PageDown), k!(Left),   k!(Down),   k!(Right),           k!(Delete),              k!(Backspace)],
            [_______,       k!(Undo),   k!(Cut),    k!(Copy),   k!(Paste),  _______,    XXXXXXX,    _______,    XXXXXXX,      wm!(Left, GUI),     XXXXXXX,    wm!(Right, GUI),     XXXXXXX,                 _______],
            // See SYM_MAC: left thumb reaches ADJUST from here
            [k!(AudioMute), _______,    _______,    _______,    ltp!(6, Delete, HT), _______, XXXXXXX, _______, _______,      _______,            _______,    _______,             _______,                 XXXXXXX],
        ],
        // 6: ADJUST
        [
            [shifted!(Grave), user!(0), user!(1),   user!(2),   user!(3),   user!(4),   XXXXXXX,    k!(MouseUp),    k!(F6),     k!(F7),     k!(F8),             k!(F9),     k!(F10),                _______],
            [_______,       BT_CLR,     DONGLE,     _______,    _______,    _______,    XXXXXXX,    k!(MouseDown),  _______,    _______,    k!(F11),            k!(F12),    shifted!(Minus),        shifted!(Equal)],
            [_______,       kbctrl!(OutputUsb), tg!(3), kbctrl!(OutputBluetooth), _______, _______, XXXXXXX, k!(MouseLeft), _______, _______, _______,          _______,    shifted!(LeftBracket),  shifted!(RightBracket)],
            // No &soft_off equivalent in RMK
            [_______,       kbctrl!(Reboot), _______, kbctrl!(Bootloader), _______, _______, XXXXXXX, k!(MouseRight), _______, _______, kbctrl!(Reboot),    XXXXXXX,    kbctrl!(Bootloader),    _______],
            [_______,       _______,    _______,    _______,    _______,    _______,    XXXXXXX,    k!(MouseBtn1),  _______,    _______,    _______,            _______,    _______,                XXXXXXX],
        ],
    ]
}

/// Left encoder: volume on the base layers, scroll everywhere else
#[rustfmt::skip]
pub const fn get_default_encoder_map() -> [[EncoderAction; NUM_ENCODER]; NUM_LAYER] {
    const VOLUME: EncoderAction = encoder!(k!(AudioVolUp), k!(AudioVolDown));
    const SCROLL: EncoderAction = encoder!(k!(MouseWheelDown), k!(MouseWheelUp));
    [
        [VOLUME], // WIN
        [SCROLL], // SYMBOL
        [SCROLL], // NAV
        [VOLUME], // MAC
        [SCROLL], // SYM_MAC
        [SCROLL], // NAV_MAC
        [SCROLL], // ADJUST
    ]
}
