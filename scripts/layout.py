"""Physical layout of the Eyelash Sofle, shared by gen_vial.py and gen_keymap_drawer.py."""

# (x, y[, r, rx, ry]) in ZMK units (100 = 1u, r in 1/100 degrees clockwise), in matrix-transform order.
# The four inner thumb keys are rotated around (rx, ry), as in ZMK's physical layout.
POS = [
    (0, 50), (100, 50), (200, 12), (300, 0), (400, 12), (500, 12), (775, 12),
    (1050, 12), (1150, 12), (1250, 0), (1350, 12), (1450, 50), (1550, 50),
    (0, 150), (100, 150), (200, 112), (300, 100), (400, 112), (500, 112), (775, 212),
    (1050, 112), (1150, 112), (1250, 100), (1350, 112), (1450, 150), (1550, 150),
    (0, 250), (100, 250), (200, 212), (300, 200), (400, 212), (500, 212), (675, 112),
    (1050, 212), (1150, 212), (1250, 200), (1350, 212), (1450, 250), (1550, 250),
    (0, 350), (100, 350), (200, 312), (300, 300), (400, 312), (500, 312), (875, 112),
    (1050, 312), (1150, 312), (1250, 300), (1350, 312), (1450, 350), (1550, 350),
    (650, 338), (200, 412), (300, 400), (400, 412),
    (550, 412, 3000, 500, 412), (675, 362, 3000, 500, 412), (775, 112),
    (875, 362, -3000, 1150, 412), (1000, 412, -3000, 1150, 412),
    (1150, 412), (1250, 400), (1350, 412),
]

# (row, col) of each POS entry. Col 6 and row 4 col 13 are unused (see dongle/src/keymap.rs).
MATRIX = []
for r in range(5):
    cols = list(range(0, 6)) + list(range(7, 13 if r == 4 else 14))
    MATRIX += [(r, c) for c in cols]
assert len(MATRIX) == len(POS) == 64


def keys():
    """Yield (row, col, x, y, r, rx, ry) per key, in matrix-transform order, in ZMK units."""
    for pos, (row, col) in zip(POS, MATRIX):
        x, y, r, rx, ry = (*pos, 0, 0, 0)[:5]
        yield row, col, x, y, r, rx, ry
