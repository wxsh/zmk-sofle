"""Generate keymap-drawer input from dongle/src/keymap.rs.

Usage: python3 scripts/gen_keymap_drawer.py
Then:  keymap -c keymap-drawer/config.yaml draw keymap-drawer/eyelash_sofle.yaml -o keymap-drawer/eyelash_sofle.svg

Writes keymap-drawer/eyelash_sofle.yaml (the layers) and keymap-drawer/eyelash_sofle.json (the physical
layout, from layout.py). Legends show what the key types with the host set to the Norwegian layout: WIN,
SYMBOL, NAV and ADJUST as on Windows, MAC, SYM_MAC and NAV_MAC as on macOS. A keycode that isn't in the
tables below fails the script, so add it here when the keymap gets a new kind of key.
"""
import json
import re
import sys
from pathlib import Path

from layout import MATRIX, keys

ROOT = Path(__file__).resolve().parent.parent
KEYMAP_RS = ROOT / "dongle/src/keymap.rs"
OUT_DIR = ROOT / "keymap-drawer"
MAC_LAYERS = {"MAC", "SYM_MAC", "NAV_MAC"}


def g(name):
    return f"$$mdi:{name}$$"


TRANS = {"t": g("transfer"), "type": "trans"}
NONE = {"t": g("minus-circle-outline"), "type": "none"}
HELD = {"type": "held"}

# Unshifted HID keys whose legend doesn't depend on the host OS
KEYS = {
    "Escape": g("keyboard-esc"),
    "Tab": g("keyboard-tab"),
    "Enter": g("keyboard-return"),
    "Space": g("keyboard-space"),
    "Backspace": g("backspace"),
    "Delete": g("backspace-reverse-outline"),
    "Up": g("arrow-up-bold"),
    "Down": g("arrow-down-bold"),
    "Left": g("arrow-left-bold"),
    "Right": g("arrow-right-bold"),
    "PageUp": "PgUp",
    "PageDown": "PgDn",
    "Home": "Home",
    "End": "End",
    "Insert": "Ins",
    "PrintScreen": g("printer-outline"),
    "Menu": "Menu",
    "Undo": "Undo",
    "Cut": "Cut",
    "Copy": "Copy",
    "Paste": "Paste",
    "AudioMute": g("volume-off"),
    "LCtrl": g("apple-keyboard-control"),
    "LShift": g("apple-keyboard-shift"),
    "Comma": {"t": ",", "s": ";"},
    "Dot": {"t": ".", "s": ":"},
    "MouseUp": {"t": g("menu-up"), "s": g("mouse")},
    "MouseDown": {"t": g("menu-down"), "s": g("mouse")},
    "MouseLeft": {"t": g("menu-left"), "s": g("mouse")},
    "MouseRight": {"t": g("menu-right"), "s": g("mouse")},
    "MouseBtn1": g("mouse-left-click-outline"),
}
KEYS |= {c: c for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ"}
KEYS |= {f"Kc{n}": str(n) for n in range(10)}
KEYS |= {f"F{n}": f"F{n}" for n in range(1, 25)}

# Keys that differ between the Windows and macOS Norwegian layouts
OS_KEYS = {
    "win": {
        "LGui": g("microsoft-windows"),
        "RGui": g("microsoft-windows"),
        "LAlt": "Alt",
        "RAlt": "AltGr",
        "Grave": {"t": "|", "s": "§"},
    },
    "mac": {
        "LGui": g("apple-keyboard-command"),
        "RGui": g("apple-keyboard-command"),
        "LAlt": g("apple-keyboard-option"),
        "RAlt": g("apple-keyboard-option"),
        "Grave": {"t": "<", "s": ">"},
    },
}

# shifted!(X): what Shift+X types on the Norwegian layout
SHIFTED = {
    "win": {"Grave": "§", "Minus": "?", "Equal": "`", "LeftBracket": "Å", "RightBracket": "^"},
    "mac": {"Grave": ">", "Minus": "?", "Equal": "`", "LeftBracket": "Å", "RightBracket": "^"},
}

# Symbols from dongle/src/nb.rs (same names in win:: and mac::)
NB = {
    "A_RING": "Å", "AE": "Æ", "O_SLASH": "Ø", "PLUS": "+", "MINUS": "-", "APOS": "'", "PIPE": "|",
    "BSLH": "\\", "LT": "<", "GT": ">", "EXCL": "!", "HASH": "#", "LPAR": "(", "RPAR": ")", "EQUAL": "=",
    "ASTRK": "*", "CARET": "^", "CIRCUMFLEX": "ˆ", "SEMI": ";", "COLON": ":", "AT": "@", "LBRC": "{",
    "RBRC": "}", "LBKT": "[", "RBKT": "]", "TILDE": "~", "ACUTE": "´",
}

MODS = {
    "win": {"CTRL": "Ctrl", "ALT": "Alt", "GUI": "Win"},
    "mac": {"CTRL": "Ctrl", "ALT": "Opt", "GUI": "Cmd"},
}

# Named KeyActions defined in keymap.rs
CONSTS = {
    "_______": TRANS,
    "XXXXXXX": NONE,
    "BT_CLR": {"t": g("bluetooth-off"), "h": "clear"},
    "CLR_PEER": {"t": "CLR PEER", "h": "hold 5s"},
    "GUI_ALT": {"t": g("apple-keyboard-option"), "s": "Cmd"},
    "GUI_SHIFT_ALT": {"t": g("apple-keyboard-option"), "s": "Cmd+Shift"},
}

KBCTRL = {
    "CapsWordToggle": {"t": g("alpha-w-box"), "h": "caps word"},
    "OutputUsb": {"t": g("usb"), "h": "output"},
    "OutputBluetooth": {"t": g("bluetooth"), "h": "output"},
    "Reboot": {"t": g("backup-restore"), "h": "reboot"},
    "Bootloader": {"t": g("progress-download"), "h": "bootloader"},
}

ENCODER = {
    ("AudioVolUp", "AudioVolDown"): "enc: vol",
    ("MouseWheelDown", "MouseWheelUp"): "enc: scrl",
}


def fail(msg):
    sys.exit(f"gen_keymap_drawer: {msg}")


def split_args(s):
    """Split on commas outside parentheses/brackets."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def macro(token):
    """`name!(a, b)` -> ("name", ["a", "b"]), else None."""
    if m := re.fullmatch(r"(\w+)!\((.*)\)", token):
        return m[1], split_args(m[2])
    return None


def as_key(legend):
    return dict(legend) if isinstance(legend, dict) else {"t": legend}


def legend(token, os, layer_names):
    if token in CONSTS:
        return as_key(CONSTS[token])
    if m := re.fullmatch(r"(win|mac)::(\w+)", token):
        if m[2] not in NB:
            fail(f"no legend for {token}, add it to NB")
        return {"t": NB[m[2]]}
    if not (mc := macro(token)):
        fail(f"can't parse {token!r}")
    name, args = mc
    if name == "k":
        (key,) = args
        if key in OS_KEYS[os]:
            return as_key(OS_KEYS[os][key])
        if key in KEYS:
            return as_key(KEYS[key])
        fail(f"no legend for {token}, add it to KEYS or OS_KEYS")
    if name == "shifted":
        (key,) = args
        if key not in SHIFTED[os]:
            fail(f"no legend for {token}, add it to SHIFTED")
        return {"t": SHIFTED[os][key]}
    if name == "ltp":
        layer, tap, _profile = args
        return {**legend(f"k!({tap})", os, layer_names), "h": layer_names[int(layer)]}
    if name == "wm":
        key, mod = args
        return {**legend(f"k!({key})", os, layer_names), "s": MODS[os][mod]}
    if name == "tg":
        return {"t": layer_names[int(args[0])], "h": "toggle"}
    if name == "kbctrl":
        if args[0] not in KBCTRL:
            fail(f"no legend for {token}, add it to KBCTRL")
        return as_key(KBCTRL[args[0]])
    if name == "user":
        n = int(args[0])
        if n <= 4:  # BLE profiles, see keymap.rs
            return {"t": g("bluetooth-connect"), "s": f"BT{n}", "h": "5s: clr"}
        fail(f"no legend for {token}")
    fail(f"no legend for {token}")


def parse_keymap(src):
    body = src[src.index("fn get_default_keymap") : src.index("fn get_default_encoder_map")]
    layers = {}
    for line in body.splitlines():
        if m := re.match(r"\s*// (\d+): (\w+)", line):
            assert int(m[1]) == len(layers), f"layer comment out of order: {line.strip()}"
            current = layers.setdefault(m[2], [])
        elif m := re.fullmatch(r"\s*\[(.+)\],\s*", line):
            row = split_args(m[1])
            assert len(row) == 14, f"row with {len(row)} keys: {line.strip()}"
            current.append(row)
    for name, rows in layers.items():
        assert len(rows) == 5, f"layer {name} has {len(rows)} rows"
    return layers


def parse_encoders(src, layer_names):
    body = src[src.index("fn get_default_encoder_map") :]
    actions = {
        m[1]: (m[2], m[3]) for m in re.finditer(r"const (\w+): EncoderAction = encoder!\(k!\((\w+)\), k!\((\w+)\)\)", body)
    }
    per_layer = re.findall(r"\[(\w+)\],", body)
    assert len(per_layer) == len(layer_names), "encoder map doesn't have one entry per layer"
    out = {}
    for layer, action in zip(layer_names, per_layer):
        if actions[action] not in ENCODER:
            fail(f"no legend for encoder {action}, add it to ENCODER")
        out[layer] = ENCODER[actions[action]]
    return out


def tri_layers(src):
    return [tuple(int(x) for x in m) for m in re.findall(r"TRI_LAYER\w*: \[u8; 3\] = \[(\d+), (\d+), (\d+)\]", src)]


def main():
    src = KEYMAP_RS.read_text()
    layers = parse_keymap(src)
    names = list(layers)
    encoders = parse_encoders(src, names)

    # Physical key index of each layer's hold-tap activators, for drawing them as held
    activators = {}
    for rows in layers.values():
        for i, (r, c) in enumerate(MATRIX):
            if (mc := macro(rows[r][c])) and mc[0] == "ltp":
                activators.setdefault(int(mc[1][0]), set()).add(i)
    held = {names[layer]: set(keys) for layer, keys in activators.items()}
    for a, b, target in tri_layers(src):
        held.setdefault(names[target], set()).update(activators.get(a, set()) | activators.get(b, set()))

    # Matrix position of the encoder push button (unrotated key in the middle of the left half)
    enc_index = MATRIX.index((4, 0))

    out_layers = {}
    for name, rows in layers.items():
        os = "mac" if name in MAC_LAYERS else "win"
        drawn = []
        for i, (r, c) in enumerate(MATRIX):
            key = HELD if i in held.get(name, ()) else legend(rows[r][c], os, names)
            if i == enc_index:
                key = {**key, "s": encoders[name]}
            drawn.append(key)
        for r, row in enumerate(rows):  # unused matrix positions must stay empty
            for c, token in enumerate(row):
                if (r, c) not in MATRIX and token != "XXXXXXX":
                    fail(f"{name}: unused position ({r}, {c}) has {token}")
        out_layers[name] = drawn

    info = {
        "keyboard_name": "Eyelash Sofle",
        "layouts": {
            "LAYOUT": {
                "layout": [
                    {"matrix": [r, c], "x": x / 100, "y": y / 100}
                    | ({"r": rot / 100, "rx": rx / 100, "ry": ry / 100} if rot else {})
                    for r, c, x, y, rot, rx, ry in keys()
                ]
            }
        },
    }
    (OUT_DIR / "eyelash_sofle.json").write_text(json.dumps(info, indent=2) + "\n")

    # JSON is valid YAML; one key per line keeps diffs readable.
    def simplify(key):
        return key["t"] if set(key) == {"t"} else key

    lines = [
        "# Generated by scripts/gen_keymap_drawer.py from dongle/src/keymap.rs. Don't edit by hand.",
        "layout: {qmk_info_json: keymap-drawer/eyelash_sofle.json}",
        "layers:",
    ]
    for name, drawn in out_layers.items():
        lines.append(f"  {name}:")
        lines += [f"  - {json.dumps(simplify(k), ensure_ascii=False)}" for k in drawn]
    (OUT_DIR / "eyelash_sofle.yaml").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
