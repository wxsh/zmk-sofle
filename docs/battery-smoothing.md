# Plan: battery level smoothing on the halves

Status: planned, not started. Roadmap item: "Battery level smoothing" in `ROADMAP.md`.

## Problem

Each half reads VDDH once every 30s, and rmk's `BatteryProcessor` turns every reading straight into a percentage,
mapped linearly from 3.6–4.2V. Nothing is averaged. With VDDH/5 one ADC count is about 4.4mV, roughly 0.7%. Normal
ADC noise plus radio-load dips of about 25mV (~6 counts) make the dongle's screen jump between values such as 73%
and 77%.

## Why smooth on the halves, not the dongle

- A half sends the dongle only a percentage (`BatteryStatusEvent`), and only when it changes (`BatteryProcessor`
  publishes when the level differs from the last one). The dongle never sees the repeated readings in between, so
  averaging there would be skewed by the jumps. The most the dongle could do is hysteresis, which hides the flicker
  but doesn't remove it.
- The halves have every raw ADC reading at full resolution.
- The halves already forward whatever `BatteryStatusEvent` is published locally (`rmk/src/split/peripheral.rs`).
  A replacement processor needs no change to the split protocol or the dongle.
- `BatteryAdcEvent(pub u16)` and `BatteryStatusEvent(pub BatteryStatus)` are public, and the `#[processor]` macro
  is exported as `rmk::macros::processor`.

## Changes

1. New `SmoothBatteryProcessor` in one shared file, `common/battery.rs`. Both halves include it with
   `#[path = "../../common/battery.rs"] mod battery;`, since the crates aren't a workspace. It replaces
   `BatteryProcessor::new(1, 5)` in `left/src/main.rs` and `right/src/main.rs`.
   - It subscribes to `BatteryAdcEvent`, as the old processor did, so it takes the old processor's subscriber
     slot and `keyboard.toml` stays as is.
   - It keeps a ring buffer of the last 12 raw readings and uses a trimmed mean that drops the min and max, so
     single radio-load dips don't count.
   - Converting to millivolts (VDDH/5: `mV = raw * 5 * 600 * 6 / 4096`) and then to percent keeps the same
     3.6–4.2V linear mapping as rmk, so levels look the same as now, just steadier.
   - It sends the first reading right away so the screen isn't blank after boot. After that it sends only when
     the smoothed level is at least 2% away from the last one sent.
2. ADC interval from 30s to 10s in `NrfAdc::new` on both halves. Twelve samples then cover two minutes. One SAADC
   sample per 10s costs next to nothing in power.
3. A `debug!` log of the raw count, smoothed mV and published %, to check it with the left half's logging.
4. Docs: remove the item from `ROADMAP.md`, and add a line to `CLAUDE.md` about `common/battery.rs`.

## Trade-offs

- Both halves need reflashing; the dongle doesn't change.
- The displayed level reacts with a delay of about a minute and moves in steps of 2% or more.
- When a half is plugged into USB, VDDH reads above 4.2V and shows 100% after about a minute. The halves have no
  charge-detect pin configured, so nothing changes about charging.
- The sandbox can't compile. CI checks the build, and behaviour has to be checked on hardware.

## Alternative

Hysteresis on the dongle alone, in `ScreenRenderer` (`dongle/src/screen.rs`): redraw a half's level only when it
moves 3% or more away from the level on screen. This needs only a dongle flash, but the reported value stays noisy
and the screen lags by up to 3%.

## Out of scope

A non-linear LiPo discharge curve. The linear 3.6–4.2V mapping overstates the middle of the range, but that's a
separate change.
