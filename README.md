Personal [RMK](https://github.com/haobogu/rmk) firmware (v0.9.0) for the Eyelash Sofle with a XIAO BLE dongle.
Ported from the ZMK config on the `Dongle` branch.

## Layout

| Crate     | Board                  | Role                                                     |
|-----------|------------------------|----------------------------------------------------------|
| `dongle/` | Seeed XIAO BLE         | Split central: keymap, Vial, USB to the host, screen     |
| `left/`   | Eyelash Sofle left     | Split peripheral 0: left matrix, encoder                 |
| `right/`  | Eyelash Sofle right    | Split peripheral 1: right matrix                         |

The keymap is in `dongle/src/keymap.rs`, the Norwegian keycodes in `dongle/src/nb.rs`.
`keyboard.toml` only holds compile-time constants (BLE profile and split peripheral counts, sleep
timeout).

## Build

CI builds UF2 images on every push (artifact `firmware`). Locally:

```shell
sudo apt install gcc-arm-none-eabi   # macOS: brew install --cask gcc-arm-embedded
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-make cargo-binutils cargo-hex-to-uf2
cargo make uf2   # -> firmware/eyelash_sofle_{left,right,dongle}.uf2
```

## Flash and pair

1. Double-tap reset on each board and copy its UF2 to the drive that shows up.
   Flashing new firmware wipes the board's settings (RMK resets storage when the
   build changes), so Vial changes are lost.
2. Plug in the dongle. The halves find it on their own: each one advertises its
   peripheral id, and the dongle saves its address.
3. To re-pair, hold `CLR_PEER` on the ADJUST layer (SYM+NAV, second key on the
   second row) for 5s. The dongle forgets both halves and picks them up again.

The `Bootloader`/`Reboot` keys only act on the dongle; use the reset button for
the halves.

RMK overwrites the SoftDevice that ZMK relies on. To go back to ZMK, build it for
the `//zmk` board variant, see the
[RMK FAQ](https://rmk.rs/docs/getting_started/faq#zmk-no-longer-works-after-flashing-rmk).

## Differences from the ZMK config

- The halves only talk to the dongle; they can't connect to a host directly.
- The dongle also advertises as a BLE keyboard (`BT0`–`BT4` select its profiles); RMK 0.9 can't turn
  that off.
- The dongle screen shows output, layer, modifiers and both batteries; its brightness is fixed at 80%
  (off after 5 minutes idle). No RGB underglow or backlight on the halves (both are switched off).
- MAC tri-layer: RMK has a single tri-layer (used for WIN). On SYM_MAC/NAV_MAC
  the other thumb key is a layer-tap to ADJUST instead.
- `BT_CLR_ALL` is replaced by `CLR_PEER`; hold a `BT0`–`BT4` key for 5s to clear that profile.
- No `&soft_off` equivalent.
- Keymap editing uses [Vial](https://get.vial.today/) instead of ZMK Studio. Unlock: hold Esc + the top-right key.
