# Patched crates

Applied with `[patch.crates-io]` in `dongle/Cargo.toml`. Only `dongle` runs the keymap, so `left` and `right` use
the upstream crates.

## `rmk-types` 0.4.0 (RMK v0.9.0)

The published crate (`rmk-types-0.4.0.crate`, sha256 matching the original `left/Cargo.lock` checksum
`e99d7952…`), unpacked as is except for `src/keycode/hid.rs`.

RMK hardcodes Caps Word's key lists for the US layout. With the host on Norwegian, that meant `-` (HID Slash) ended
Caps Word, `+` (HID Minus) was shifted to `?`, and æ/ø/å ended it without being capitalised. The patch changes
`is_caps_word_shifted_key` and `is_caps_word_continue_key`:

- Shifted (and continue): A–Z, å (LeftBracket), ø (Semicolon), æ (Quote), `-` → `_` (Slash).
- Continue only: 1–0, Backspace, Delete.
- Everything else, including `+` (HID Minus), ends Caps Word.

When upgrading RMK, unpack the new `rmk-types` version here and reapply the change (or drop the patch if upstream
makes the lists configurable).
