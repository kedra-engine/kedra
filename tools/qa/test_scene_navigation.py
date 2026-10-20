"""Headless checks that native Scene View QA rejects clipped/misanchored UI."""
import copy
import unittest

from scene_navigation import anchored_menu, toolbar_geometry


class Observation:
    def __init__(self):
        self.nodes = {
            "scene/viewport": {"rect": [0, 108, 240, 500], "window": 7},
            "scene/projection": {"rect": [8, 8, 136, 34], "window": 7, "entity": 1},
            "scene/lighting": {"rect": [142, 8, 230, 34], "window": 7, "entity": 2},
            "scene/grid": {"rect": [8, 40, 34, 66], "window": 7, "entity": 3},
            "scene/views": {"rect": [8, 72, 34, 98], "window": 7, "entity": 4},
            "dock-menu": {"rect": [8, 38, 200, 100], "window": 7, "parent": 1},
        }

    def node(self, key):
        return self.nodes.get(key)

    def observe(self):
        return {"windows": [{"id": 7, "width": 322, "height": 640}]}


class GeometryTests(unittest.TestCase):
    def test_wrapped_targets_and_native_anchor_are_accepted(self):
        c = Observation()
        toolbar_geometry(c, "scene/")
        anchored_menu(c, "scene/", "projection")

    def test_visible_but_clipped_or_overlapping_targets_are_rejected(self):
        original = Observation()
        for field, value in (("rect", [220, 40, 246, 66]),
                             ("rect", [8, 100, 34, 126]),
                             ("window", 8)):
            c = copy.deepcopy(original)
            c.nodes["scene/views"][field] = value
            with self.assertRaises(AssertionError):
                toolbar_geometry(c, "scene/")

    def test_detached_stale_or_offscreen_menu_is_rejected(self):
        original = Observation()
        for field, value in (("parent", 4), ("window", 8),
                             ("rect", [8, 200, 200, 260]),
                             ("rect", [200, 38, 400, 100])):
            c = copy.deepcopy(original)
            c.nodes["dock-menu"][field] = value
            with self.assertRaises(AssertionError):
                anchored_menu(c, "scene/", "projection")


if __name__ == "__main__":
    unittest.main()
