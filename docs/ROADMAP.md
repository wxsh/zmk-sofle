# Roadmap: ZMK features not yet ported

These are things the ZMK config (branch `Dongle`) had that the RMK port doesn't have yet. Rough order
is by impact. "ZMK reference" points at files on the `Dongle` branch.

## Hardware support

### Dongle screen
The XIAO dongle drove a colour LCD through `zmk-dongle-screen` (janpfischer, v0.1.0). It showed:
- Active layer, modifiers and output
- Battery levels of both halves
- Brightness controlled from the keyboard (keycodes 115/114, 5% steps, 10–80%, off after 300s idle)

What a port needs:
- RMK's `lcd_async` display driver (supports ST7789) and a custom `Rgb565` `DisplayRenderer`.
  The built-in renderers are monochrome only.
- A source for the keyboard state. In RMK's relay topology the dongle only sees HID reports, not the
  layer or the halves' batteries. That requires either the dongle-as-central topology below, or
  forwarding state from the left half over BLE (not in RMK 0.9).

ZMK reference: `config/eyelash_sofle.conf` (`CONFIG_DONGLE_SCREEN_*`), `config/west.yml`.

### RGB underglow
- 7 WS2812 LEDs, data on P1.12 through SPI3 (GRB order), powered through EXT_POWER (P0.13).
- ZMK settings: off at start, hue 160, effect 3, max brightness 90, auto-off on idle and on USB.
- RMK 0.9 has no WS2812/underglow support. It would need a custom driver (SPI-encoded WS2812) and a
  processor reacting to light keycodes. P0.13 is currently held low.

ZMK reference: `boards/arm/eyelash_sofle/eyelash_sofle.dtsi` (`led_strip`), `config/eyelash_sofle.conf`.

### Backlight
- PWM backlight on P1.13 (PWM0, 1ms period). ZMK turned it on at boot at 100% and never idled it off.
- RMK 0.9 has the `light!(Backlight*)` keycodes but no PWM backlight driver. Needs a custom
  processor. P1.13 is currently held low.

## Topology

### Dongle as split central
- In ZMK the dongle ran the keymap and both halves were peripherals.
- In RMK the left half is the central and the dongle only relays it, so the left half's battery
  drains faster.
- RMK 0.9 has no matrix-less central. A port would need a central with an empty matrix and two
  peripherals, which isn't supported upstream.
- This would also give the dongle screen full keyboard state.

## Keymap behaviour

### Exact MAC tri-layer
ZMK used conditional layers: 1+2→6 (WIN) and 4+5→6 (MAC). RMK supports a single `tri_layer`, so MAC
uses `ltp!(6, …)` on the opposite thumb. This differs from ZMK in two ways:
- Transparent ADJUST keys fall through to whichever MAC layer was held first.
- Releasing the first thumb keeps ADJUST active.

Needs upstream support for multiple tri-layers/conditional layers, or a public layer API so a
processor can drive them (`KeyMap::activate_layer` is `pub(crate)` in 0.9).

### `hold-while-undecided` on thumb hold-taps
- ZMK activated the layer immediately while the hold-tap was undecided. RMK has no equivalent.
- The rest of `my_mt` is ported: hold-preferred, 150ms tapping term, 150ms quick-tap.

### Soft off
ZMK's `&soft_off` on ADJUST has no RMK equivalent; the key is currently `XXXXXXX`.

### Clear all BT bonds
- ZMK's `BT_CLR_ALL` has no RMK equivalent. That slot is now `DONGLE`.
- To clear a profile, hold its `BT0`–`BT4` key for 5s.

### Bootloader/reset on the right half
- ZMK runs `&bootloader`/`&sys_reset` on the half where the key is pressed.
- In RMK these actions only reach the left half. The right-hand keys on ADJUST now reboot the left half.

### Caps Word with the Norwegian layout
RMK's Caps Word keeps going on US `-` (HID Minus), which is `+` on Norwegian. Norwegian `-` is HID
Slash, which probably ends Caps Word and isn't shifted to `_`. Not verified yet.

### Mouse key and scroll tuning
ZMK's settings:
- Move: 1200 px/s, scaled 2x, linear acceleration over 500ms, 16ms trigger period.
- Scroll: value 25, scaled 2x.
- The encoder scrolled with a 100ms tap.

The RMK values in `left/src/main.rs` are approximations and need tuning on hardware.

## Tooling

### Keymap drawing
- The `draw.yml` workflow (keymap-drawer) parsed the ZMK keymap and was removed.
- `keymap-drawer/*.svg` are now static snapshots of the ZMK keymap.
- Needs a generator from `keymap.rs` (or Vial JSON) to keymap-drawer YAML.

### Settings reset firmware
ZMK built `settings_reset` UF2s for each board to clear bonds/settings. The RMK port has no
equivalent build.
