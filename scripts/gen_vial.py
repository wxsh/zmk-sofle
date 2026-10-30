"""Generate left/vial.json for the Eyelash Sofle from the ZMK physical layout.

Usage: python3 scripts/gen_vial.py left/vial.json

Edit this script rather than vial.json by hand. Keep `custom` in sync with the
BLE user keys (it depends on ble_profiles_num in keyboard.toml).
"""
import json
import sys

# (x, y) in ZMK units (100 = 1u), in matrix-transform order
POS = [
    (0, 50), (100, 50), (200, 12), (300, 0), (400, 12), (500, 12), (775, 12),
    (1050, 12), (1150, 12), (1250, 0), (1350, 12), (1450, 50), (1550, 50),
    (0, 150), (100, 150), (200, 112), (300, 100), (400, 112), (500, 112), (775, 212),
    (1050, 112), (1150, 112), (1250, 100), (1350, 112), (1450, 150), (1550, 150),
    (0, 250), (100, 250), (200, 212), (300, 200), (400, 212), (500, 212), (675, 112),
    (1050, 212), (1150, 212), (1250, 200), (1350, 212), (1450, 250), (1550, 250),
    (0, 350), (100, 350), (200, 312), (300, 300), (400, 312), (500, 312), (875, 112),
    (1050, 312), (1150, 312), (1250, 300), (1350, 312), (1450, 350), (1550, 350),
    (650, 338), (200, 412), (300, 400), (400, 412), (550, 412), (675, 362), (775, 112),
    (875, 362), (1000, 412), (1150, 412), (1250, 400), (1350, 412),
]

MATRIX = []
for r in range(5):
    cols = list(range(0, 6)) + list(range(7, 13 if r == 4 else 14))
    MATRIX += [(r, c) for c in cols]
assert len(MATRIX) == len(POS) == 64

# One KLE row per key: x resets to 0 each row, y advances by 1 per row
rows = []
cursor_y = 0.0
for (x, y), (r, c) in zip(POS, MATRIX):
    x, y = x / 100, y / 100
    rows.append([{"x": x, "y": round(y - cursor_y, 2)}, f"{r},{c}"])
    cursor_y = y + 1

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
    ("CLR_PEER", "Forget the bonded split peer (hold 5s)"),
    ("DONGLE", "Switch to the dongle (hold 5s to re-pair)"),
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
