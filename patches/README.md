# Patched crates

Applied with `[patch.crates-io]` in `dongle/Cargo.toml`. Only `dongle` runs the keymap, so `left` and `right` use
the upstream crates. Neither patch changes the split messages, so the halves stay compatible.

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

## `rmk` 0.9.0

The published crate (`rmk-0.9.0.crate`, sha256 `b6e673c9…`, matching `dongle/Cargo.lock`), unpacked as is except
for these changes, marked `PATCHED`:

- `src/config/behavior.rs`: `BehaviorConfig::tri_layer_2`, a second tri-layer.
- `src/keymap.rs`: `update_tri_layer` applies both. The upper layer is on while either pair of layers is held.
- `src/keyboard.rs`: `rmk::keyboard::caps_word_active()`, for the Caps Word indicator on the dongle's screen. Caps
  Word's state is private to `Keyboard` and no event carries it, so `CapsWordState` stores when it times out in a
  static that the function reads.

RMK supports a single tri-layer, used by WIN (SYMBOL + NAV -> ADJUST). MAC needs its own (SYM_MAC + NAV_MAC ->
ADJUST). The workaround without a patch, `ltp!(6, …)` on the other thumb in SYM_MAC and NAV_MAC, fails when both
thumbs go down within the tapping term: RMK looks up the second key's action before it resolves the first key as a
hold, so the second thumb gets MAC's `ltp!(5, …)` and ADJUST never turns on.

When upgrading RMK, unpack the new `rmk` version here and reapply the changes (or drop them once upstream supports
several tri-layers and exposes Caps Word's state).
