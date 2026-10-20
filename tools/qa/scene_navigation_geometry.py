"""Selected C-select/C-narrow chrome geometry, independent of rendered scene pixels."""


def toolbar_geometry(c, prefix):
    viewport = c.node(prefix + "viewport")
    toolbar = c.node(prefix + "toolbar")
    assert viewport and toolbar, "Scene toolbar/viewport missing"
    bounds, bar = viewport["rect"], toolbar["rect"]
    assert abs(bar[3] - bar[1] - 38) <= 1, bar
    assert abs(bar[3] - bounds[1]) <= 1, (bar, bounds)
    targets = []
    for name in ("views", "overlays", "grid", "lighting"):
        control = c.node(prefix + name)
        assert control and control["window"] == viewport["window"], name
        rect = control["rect"]
        assert bar[0] <= rect[0] < rect[2] <= bar[2], (name, rect, bar)
        assert abs(rect[1] - bar[1] - 6) <= 1, (name, rect, bar)
        assert abs(rect[3] - rect[1] - 26) <= 1, (name, rect)
        targets.append(rect)
    assert all(a[2] <= b[0] for a, b in zip(targets, targets[1:])), targets
    if bounds[2] - bounds[0] < 640:
        assert not c.node(prefix + "select-menu"), "Narrow layout retains Select"
    if bounds[2] - bounds[0] >= 160 and bounds[3] - bounds[1] >= 230:
        orbit = c.node(prefix + "orbit")
        assert orbit, "Orientation control missing"
        r = orbit["rect"]
        assert abs(r[2] - r[0] - 96) <= 1 and abs(r[3] - r[1] - 96) <= 1, r
        assert abs(bounds[2] - r[2] - 12) <= 1 and abs(r[1] - bounds[1] - 8) <= 1, (r, bounds)
        for name in ("zoom", "pan", "frame", "projection", "select-tool"):
            node = c.node(prefix + name)
            assert node and node["window"] == viewport["window"], name
            r = node["rect"]
            assert bounds[0] <= r[0] < r[2] <= bounds[2] and bounds[1] <= r[1] < r[3] <= bounds[3], (name, r, bounds)
            assert abs(r[2] - r[0] - 26) <= 1 and abs(r[3] - r[1] - 26) <= 1, (name, r)
    else:
        for name in ("orbit", "zoom", "pan", "frame", "projection"):
            assert not c.node(prefix + name), ("Short canvas retains navigation", name)
    return viewport


def anchored_menu(c, prefix, control):
    anchor = c.node(prefix + control)
    popup = c.node("dock-menu")
    assert popup and anchor and popup["window"] == anchor["window"]
    assert popup["parent"] == anchor["entity"], "Menu lost its invoking control"
    a, p = anchor["rect"], popup["rect"]
    assert min(abs(p[1] - a[3] - 4), abs(a[1] - p[3] - 4)) <= 1, (a, p)
    assert p[0] <= a[2] and p[2] >= a[0], (a, p)
    window = next(w for w in c.observe()["windows"] if w["id"] == anchor["window"])
    assert p[0] >= 3 and p[1] >= 3, p
    assert p[2] <= window["width"] - 3 and p[3] <= window["height"] - 3, (p, window)
    return {"anchor": a, "popup": p, "window": window["id"]}


def resize_drag(group, dividers, width):
    """Pointer positions for the actual neighboring native divider."""
    candidates = [(min(abs(n["rect"][0] - group[2]), abs(n["rect"][2] - group[0])), n)
                  for n in dividers if min(n["rect"][3], group[3]) > max(n["rect"][1], group[1])]
    assert candidates, "No neighboring divider"
    distance, divider = min(candidates, key=lambda item: item[0])
    assert distance <= 8, "Scene group has no adjoining vertical divider"
    r = divider["rect"]
    point = [(r[0] + r[2]) / 2, (r[1] + r[3]) / 2]
    half = (r[2] - r[0]) / 2
    target = group[0] + width + half if abs(r[0] - group[2]) <= 8 else group[2] - width - half
    return point, [target, point[1]]
