Personal [RMK](https://github.com/haobogu/rmk) firmware (v0.9.0) for the Eyelash Sofle with a XIAO BLE dongle.
Ported from the ZMK config on the `Dongle` branch.

## Layout

| Crate     | Board                  | Role                                                     |
|-----------|------------------------|----------------------------------------------------------|
| `left/`   | Eyelash Sofle left     | Split central: keymap, left matrix, encoder, Vial        |
| `right/`  | Eyelash Sofle right    | Split peripheral                                         |
| `dongle/` | Seeed XIAO BLE         | USB relay for the left half (HID + Vial)                 |

The keymap is in `left/src/keymap.rs`, the Norwegian keycodes in `left/src/nb.rs`.
`keyboard.toml` only holds compile-time constants (BLE profile count, sleep timeout).

## Build

CI builds UF2 images on every push (artifact `firmware`). Locally:

```shell
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-make cargo-binutils cargo-hex-to-uf2
cargo make uf2   # -> firmware/eyelash_sofle_{left,right,dongle}.uf2
```

## Flash and pair

1. Double-tap reset on each board and copy its UF2 to the drive that shows up.
2. The halves find each other on their own.
3. Plug in the dongle, then press `DONGLE` on the ADJUST layer (SYM+NAV, second
   key on the second row) to pair the left half with it. Hold `DONGLE` for 5s to
   forget the dongle and pair a new one.

The `Bootloader`/`Reboot` keys only act on the left half; use the reset button
for the right half and the dongle.

RMK overwrites the SoftDevice that ZMK relies on. To go back to ZMK, build it for
the `//zmk` board variant, see the
[RMK FAQ](https://rmk.rs/docs/getting_started/faq#zmk-no-longer-works-after-flashing-rmk).

## Differences from the ZMK config

- The dongle is a relay; the keymap runs on the left half, which uses more battery.
- No dongle screen, RGB underglow or backlight (both are switched off).
- MAC tri-layer: RMK has a single tri-layer (used for WIN). On SYM_MAC/NAV_MAC
  the other thumb key is a layer-tap to ADJUST instead.
- `BT_CLR_ALL` is replaced by `DONGLE`; hold a `BT0`–`BT4` key for 5s to clear that profile.
- No `&soft_off` equivalent.
- Keymap editing uses [Vial](https://get.vial.today/) instead of ZMK Studio. Unlock: hold Esc + the top-right key.
