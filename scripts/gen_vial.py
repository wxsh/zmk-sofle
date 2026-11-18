"""Generate dongle/vial.json for the Eyelash Sofle from the ZMK physical layout.

Usage: python3 scripts/gen_vial.py dongle/vial.json

Edit this script rather than vial.json by hand (key positions are in layout.py). Keep `custom` in
sync with the BLE user keys (it depends on ble_profiles_num in keyboard.toml).
"""
import json
import sys

from layout import keys

# One KLE row per key. Every key sets r/rx/ry, which moves the KLE cursor to (rx, ry), so x/y are
# offsets from the rotation origin and no cursor state carries over between rows.
rows = []
for row, col, x, y, r, rx, ry in keys():
    props = {"r": r / 100, "rx": rx / 100, "ry": ry / 100, "x": (x - rx) / 100, "y": (y - ry) / 100}
    rows.append([props, f"{row},{col}"])

custom = [
    ("BT0", "Bluetooth profile 0"),
    ("BT1", "Bluetooth profile 1"),
    ("BT2", "Bluetooth profile 2"),
    ("BT3", "Bluetooth profile 3"),
    ("BT4", "Bluetooth profile 4"),
    ("NEXT_BT", "Next Bluetooth profile"),
    ("PREV_BT", "Previous Bluetooth profile"),
    ("CLR_BT", "Clear bond of the current profile"),
    ("SWITCH", "Switch default output between USB/BLE"),
    ("CLR_PEER", "Forget both halves so they re-pair (hold 5s)"),
]

vial = {
    "name": "Eyelash Sofle",
    "vendorId": "0x4C4B",
    "productId": "0x4643",
    "lighting": "none",
    "matrix": {"rows": 5, "cols": 14},
    "customKeycodes": [
        {"name": n, "title": t, "shortName": n.replace("_", "\n")} for n, t in custom
    ],
    "layouts": {"keymap": rows},
}

with open(sys.argv[1], "w") as f:
    json.dump(vial, f, indent=2)
    f.write("\n")
