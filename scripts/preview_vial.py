"""Render the Vial layout in dongle/vial.json as an SVG, to check it without flashing.

Usage: python3 scripts/preview_vial.py dongle/vial.json preview.svg

Each key is labelled with its matrix position (row,col). Overlapping keys are drawn in red, and
the script exits non-zero if there are any.
"""
import json
import math
import sys

U = 60  # pixels per key unit
PAD = 20


def parse_kle(rows):
    """Yield (label, corners) per key, following kle-serial's cursor and rotation rules."""
    r = rx = ry = 0.0
    cluster_x = cluster_y = 0.0
    x = y = 0.0
    w = h = 1.0
    for row in rows:
        for item in row:
            if isinstance(item, dict):
                r = item.get("r", r)
                if "rx" in item:
                    rx = cluster_x = item["rx"]
                    x, y = cluster_x, cluster_y
                if "ry" in item:
                    ry = cluster_y = item["ry"]
                    x, y = cluster_x, cluster_y
                x += item.get("x", 0)
                y += item.get("y", 0)
                w, h = item.get("w", 1), item.get("h", 1)
                continue
            a = math.radians(r)
            corners = []
            for px, py in ((x, y), (x + w, y), (x + w, y + h), (x, y + h)):
                dx, dy = px - rx, py - ry
                corners.append((rx + dx * math.cos(a) - dy * math.sin(a), ry + dx * math.sin(a) + dy * math.cos(a)))
            yield item.split("\n")[0], corners
            x += w
            w = h = 1.0
        y += 1
        x = cluster_x


def overlaps(p, q):
    """Separating axis test for two convex quads."""
    for poly in (p, q):
        for i in range(4):
            (ax, ay), (bx, by) = poly[i], poly[(i + 1) % 4]
            nx, ny = by - ay, ax - bx
            pa = [nx * u + ny * v for u, v in p]
            pb = [nx * u + ny * v for u, v in q]
            if max(pa) <= min(pb) + 1e-6 or max(pb) <= min(pa) + 1e-6:
                return False
    return True


def main(src, dst):
    keys = list(parse_kle(json.load(open(src))["layouts"]["keymap"]))
    bad = set()
    for i, (la, pa) in enumerate(keys):
        for lb, pb in keys[i + 1 :]:
            if overlaps(pa, pb):
                bad |= {la, lb}
                print(f"overlap: {la} and {lb}")

    xs = [u for _, c in keys for u, _ in c]
    ys = [v for _, c in keys for _, v in c]
    ox, oy = min(xs), min(ys)
    width = (max(xs) - ox) * U + 2 * PAD
    height = (max(ys) - oy) * U + 2 * PAD

    def pt(u, v):
        return f"{(u - ox) * U + PAD:.1f},{(v - oy) * U + PAD:.1f}"

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" height="{height:.0f}" '
        f'font-family="sans-serif" font-size="13">',
        '<rect width="100%" height="100%" fill="#fff"/>',
    ]
    for label, corners in keys:
        fill = "#f88" if label in bad else "#ddd"
        out.append(
            f'<polygon points="{" ".join(pt(u, v) for u, v in corners)}" '
            f'fill="{fill}" fill-opacity="0.8" stroke="#333"/>'
        )
        cu = sum(u for u, _ in corners) / 4
        cv = sum(v for _, v in corners) / 4
        cx, cy = pt(cu, cv).split(",")
        out.append(f'<text x="{cx}" y="{cy}" text-anchor="middle" dominant-baseline="middle">{label}</text>')
    out.append("</svg>")

    with open(dst, "w") as f:
        f.write("\n".join(out) + "\n")
    print(f"{len(keys)} keys, {len(bad)} overlapping -> {dst}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
