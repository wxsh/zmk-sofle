# Eyelash Sofle RMK firmware

RMK v0.9.0 firmware (Rust, `use_rust` style) for an Eyelash Sofle split keyboard with a Seeed XIAO BLE
USB dongle. Ported from ZMK (the `Dongle` branch has the original ZMK config). Features still missing
from the ZMK setup are tracked in `docs/ROADMAP.md`.

## Structure

Three independent crates (not a Cargo workspace, so each one gets only its own `rmk` features):

- `dongle/`: XIAO BLE split central with no matrix (as in the ZMK setup). Holds the keymap, serves
  Vial, and talks to the host over USB (RMK also makes it advertise as a BLE keyboard).
- `left/`: split peripheral 0. Matrix, encoder and battery.
- `right/`: split peripheral 1. Matrix and battery.
- Peripheral ids are the first argument to `run_rmk_split_peripheral` and index the
  `PeripheralMatrixConfig` array in `dongle/src/main.rs`. `split_peripherals_num` in `keyboard.toml`
  must match the number of halves.
- The split protocol encodes messages by variant index, and rmk's `display` feature adds variants.
  Keep `display` enabled on **all three** crates (the dongle gets it through `st7789`), or the halves
  and the dongle misread each other's messages.

- All three `build_sdc` set `.default_tx_power(8)` (+8dBm, as ZMK's `CONFIG_BT_CTLR_TX_PWR_PLUS_8`).
  At the default 0dBm the dongle-to-half link failed to establish (`0x3E` sync timeout).
- The dongle shortens the SDC's connection event length (2.5ms) and central ACL event spacing (3.75ms)
  in `set_sdc_event_timing()`. At the 7.5ms defaults the two 7.5ms split links collided every event,
  and one half kept dropping (`0x3E` on connect, `0x08` supervision timeout after).
- Late keys, key repeats and drops on one half were a faulty dongle (first XIAO): it heard both halves at
  -75 to -92 dBm even from 5cm, and a phone heard it 25dB weaker than the halves. A good XIAO reads -37 to
  -45 dBm. To check a dongle, log RSSI with `conn.rssi(stack)` in `run_central_manager_task` (needs a
  `ControllerCmdSync<ReadRssi>` bound up to `BleTransport`), or compare it with the halves in nRF Connect.
- A host that once paired with a half (the left was the BLE keyboard in the first RMK version, same address)
  keeps reconnecting to its split advertising and steals it from the dongle (`Long term key request ... no
  long term key` in the half's log). Fix: forget the old "Eyelash Sofle" pairing on the host. The halves'
  `ble_addr()` also XORs the low bytes so their addresses no longer match that old one; keep left and right alike.

## Dongle screen (`dongle/src/screen.rs`)

- 240x280 ST7789V from the ZMK `dongle_screen` shield, used in landscape. Pins are in the module doc.
- `ScreenRenderer` redraws only the parts that changed. `ScreenController` runs the backlight PWM and
  records the active transport, which RMK's `RenderContext` doesn't carry.
- RMK sizes event subscriber slots at build time. Any new `XEvent::subscriber()` in our code needs
  its slot added under `[event.x]` in `keyboard.toml`, otherwise it panics at runtime.
- RAM is tight. RMK's `LcdAsyncDisplay` (full RGB565 framebuffer, 131K) left too little stack: it
  silently overflowed into the statics (no guard), which showed up as lockup resets and a
  "`StaticCell` is already full" panic. `dongle/src/palette_display.rs` stores 4-bit indices into
  `screen::PALETTE` instead (33K); new colors must be added there. It drives the ST7789 directly
  (not through lcd-async) so that each flush is one window streamed with CS held low: the panel
  stayed black when the screen was sent as separate `show_raw_data` windows. Buffers use `ConstStaticCell` so
  they never pass through the stack.

Shared at the root:
- `.cargo/config.toml`: target and `KEYBOARD_TOML_PATH`.
- `keyboard.toml`: RMK compile-time constants only (`ble_profiles_num`, `split_peripherals_num`, sleep
  timeout).
- `memory.x`: nRF52840 with the Adafruit UF2 bootloader; flash starts at 0x1000.
- `Makefile.toml`: `cargo make uf2` builds all three UF2s into `firmware/`.
- `patches/`: crates patched via `[patch.crates-io]` in `dongle/Cargo.toml`: `rmk-types` (Norwegian Caps
  Word) and `rmk` (a second tri-layer, and `caps_word_active()` for the screen). See `patches/README.md`. Reapply both when upgrading RMK.

## Keymap conventions (`dongle/src/keymap.rs`)

- Unified 5x14 matrix: left half is cols 0-5, right half cols 7-13. **Col 6 is unused** (always
  `XXXXXXX`), and so is row 4 col 13. The halves are attached at `col_offset: 0` and `7` in
  `dongle/src/main.rs`.
- Layers: 0 WIN, 1 SYMBOL, 2 NAV, 3 MAC (toggled on top of WIN), 4 SYM_MAC, 5 NAV_MAC, 6 ADJUST.
- Both OSes reach ADJUST through a tri-layer: WIN `TRI_LAYER = [1, 2, 6]`, MAC `TRI_LAYER_MAC = [4, 5, 6]`
  (`tri_layer_2`, from the patched `rmk`). Don't put `ltp!(6, …)` on the opposite thumb instead: RMK looks a
  key up before resolving a pending hold-tap, so that fails when both thumbs go down within the tapping term.
- Thumb hold-taps use morse profile index `HT` (`HOLD_TAP_PROFILE`, which matches ZMK's `my_mt`). The
  profile is pushed into `behavior_config.morse.profiles` in `main.rs`, so its index must stay in sync.
- The host OS uses the Norwegian layout. Symbols come from `dongle/src/nb.rs` (`win::` = ZMK `NB_*`,
  `mac::` = ZMK `NBM_*`). Add new symbols there instead of using US keycodes like `k!(Slash)`.
- BLE user keys assume 5 profiles: `user!(0..=4)` select a profile (on the dongle's own BLE radio),
  7 clears the current one, 9 is `CLR_PEER` (hold 5s to forget both halves). Changing
  `ble_profiles_num` shifts these indices and the `customKeycodes` in `vial.json`.

## Vial

- `dongle/vial.json` is generated by `python3 scripts/gen_vial.py dongle/vial.json`. Edit the script (key
  positions, matrix, `customKeycodes`), not the JSON, then regenerate.
- `dongle/build.rs` compresses it and holds the Vial keyboard ID.
- Only `dongle` has the `vial` feature; the halves have no host protocol.

## Building and verifying

- CI (`.github/workflows/build.yml`) is the source of truth for whether the firmware compiles. Builds
  need `arm-none-eabi-gcc` because `p256-cortex-m4-sys` (BLE pairing) compiles C for the target.
  With it installed, the `nrf-*-sys` bindgen step needs `-ffreestanding` (set in the workflow), or
  it fails with `unknown type name 'uint32_t'` in `cmsis_gcc.h`.
- The dev sandbox has no host C linker, so `cargo build`/`cargo check` fail here. What does work:
  - `cargo generate-lockfile` (dependency resolution)
  - `rustfmt --check` (syntax)
  - Reading the RMK source at the `rmk-v0.9.0` tag.
- Format with the root `rustfmt.toml` (edition 2024, width 120). Keymap tables are `#[rustfmt::skip]`.
- Hardware facts (pins, crystal, VDDH battery, EXT_POWER on P0.13, backlight on P1.13) come from the
  deleted ZMK board files. Check `git show Dongle:boards/arm/eyelash_sofle/eyelash_sofle.dtsi`
  before changing pin assignments.
