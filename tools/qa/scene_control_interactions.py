"""Real pointer input for the C-select orientation and navigation controls."""
import math


def center(node):
    r = node["rect"]
    return [(r[0] + r[2]) / 2, (r[1] + r[3]) / 2]


def exercise(c, prefix, pose, selection, record):
    window = c.node(prefix + "viewport")["window"]
    assert next(w for w in c.observe()["windows"] if w["id"] == window)["focused"], "Native camera QA requires the actual foreground window; background input must be rejected"
    original_selection = selection()
    for axis, expected in (("x", [1, 0, 0]), ("y", [0, 1, 0]), ("z", [0, 0, 1])):
        for side in (1, -1):
            c.click(prefix + "axis-" + axis)

            def aligned():
                p = pose()
                direction = [a - b for a, b in zip(p["translation"], p["pivot"])]
                length = math.sqrt(sum(x * x for x in direction))
                return not p["aligning"] and length > 0 and all(abs(a / length - side * b) < .002 for a, b in zip(direction, expected))

            c.wait(aligned, axis + " signed alignment")
            assert selection() == original_selection, "Axis click selected through the overlay"
            record("axis-" + axis + ("-positive" if side == 1 else "-negative"))

    for name, changed in (("orbit", "rotation"), ("zoom", "translation"), ("pan", "pivot")):
        node = c.node(prefix + name)
        point = center(node)
        before = pose()
        c.event(window=window, kind="move", position=point)
        c.event(window=window, kind="button", pressed=True)
        c.wait(lambda: c.state()["navigation_input"]["capture"], name + " capture")
        c.event(window=window, kind="move", position=[point[0] + 18, point[1] + 12])
        c.wait(lambda: pose()[changed] != before[changed], name + " changes camera")
        record(name + "-captured")
        c.event(window=window, kind="button", pressed=False)
        c.wait(lambda: c.state()["navigation_input"]["capture"] is None, name + " release")
        assert selection() == original_selection, name + " selected through the overlay"
        stopped = pose()
        c.event(window=window, kind="move", position=[point[0] + 24, point[1] + 16])
        assert pose() == stopped, name + " continued after release"

        # The same gesture must cancel on Escape, without a release-triggered snap.
        point = center(c.node(prefix + name))
        c.event(window=window, kind="move", position=point)
        c.event(window=window, kind="button", pressed=True)
        c.wait(lambda: c.state()["navigation_input"]["capture"], name + " second capture")
        c.event(window=window, kind="key", key="escape")
        c.wait(lambda: c.state()["navigation_input"]["capture"] is None, name + " Escape")
        stopped = pose()
        c.event(window=window, kind="move", position=[point[0] + 12, point[1] + 12])
        c.event(window=window, kind="button", pressed=False)
        assert pose() == stopped, name + " continued after Escape"
        assert selection() == original_selection

    before = pose()
    c.click(prefix + "overlays")
    c.wait(lambda: not pose()["overlays"], "hide selection bounds")
    assert selection() == original_selection
    record("overlays-hidden")
    c.click(prefix + "overlays")
    c.wait(lambda: pose()["overlays"], "restore selection bounds")
    assert pose() == before and selection() == original_selection
    c.click(prefix + "projection")
    c.wait(lambda: pose()["orthographic"] != before["orthographic"], "projection button")
    assert selection() == original_selection
    c.click(prefix + "projection")
    c.wait(lambda: pose()["orthographic"] == before["orthographic"], "projection restore")
    record("navigation-controls-complete")
