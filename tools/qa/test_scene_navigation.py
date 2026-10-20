"""Headless checks that native Scene View QA rejects clipped/misanchored UI."""
import copy
import unittest

from scene_navigation import anchored_menu, toolbar_geometry
from scene_navigation_geometry import resize_drag


class Observation:
    def __init__(self, legacy=False):
        self.nodes = {
            "scene/viewport": {"rect": [0, 108, 240, 500], "window": 7},
            "scene/projection": {"rect": [8, 8, 136, 34], "window": 7, "entity": 1},
            "scene/lighting": {"rect": [142, 8, 230, 34], "window": 7, "entity": 2},
            "scene/grid": {"rect": [8, 40, 34, 66], "window": 7, "entity": 3},
            "scene/views": {"rect": [8, 72, 34, 98], "window": 7, "entity": 4},
            "dock-menu": {"rect": [8, 38, 200, 100], "window": 7, "parent": 1},
        }
        if not legacy:
            self.nodes = {
                "scene/viewport": {"rect": [0, 38, 480, 360], "window": 7},
                "scene/toolbar": {"rect": [0, 0, 480, 38], "window": 7},
                "scene/views": {"rect": [6, 6, 50, 32], "window": 7, "entity": 4},
                "scene/overlays": {"rect": [380, 6, 406, 32], "window": 7, "entity": 5},
                "scene/grid": {"rect": [408, 6, 434, 32], "window": 7, "entity": 3},
                "scene/lighting": {"rect": [448, 6, 474, 32], "window": 7, "entity": 2},
                "scene/orbit": {"rect": [372, 46, 468, 142], "window": 7},
                "scene/zoom": {"rect": [442, 146, 468, 172], "window": 7},
                "scene/pan": {"rect": [442, 174, 468, 200], "window": 7},
                "scene/frame": {"rect": [442, 208, 468, 234], "window": 7},
                "scene/projection": {"rect": [442, 242, 468, 268], "window": 7},
                "scene/select-tool": {"rect": [14, 56, 40, 82], "window": 7},
                "dock-menu": {"rect": [6, 36, 200, 100], "window": 7, "parent": 4},
            }

    def node(self, key):
        return self.nodes.get(key)

    def observe(self):
        return {"windows": [{"id": 7, "width": 500, "height": 640}]}


class GeometryTests(unittest.TestCase):
    def test_redock_to_either_outer_group_uses_its_actual_neighboring_divider(self):
        dividers = [{"rect": [288, 63, 292, 899]}, {"rect": [1149.5, 63, 1153.5, 899]}]
        self.assertEqual(resize_drag([1, 63, 288, 899], dividers, 240), ([290, 481], [243, 481]))
        self.assertEqual(resize_drag([1153.5, 63, 1439.5, 899], dividers, 240), ([1151.5, 481], [1197.5, 481]))

    def test_retired_wrapping_toolbar_does_not_count_as_blender_layout(self):
        with self.assertRaises(AssertionError):
            toolbar_geometry(Observation(legacy=True), "scene/")

    def test_selected_narrow_layout_and_native_anchor_are_accepted(self):
        c = Observation()
        toolbar_geometry(c, "scene/")
        anchored_menu(c, "scene/", "views")

    def test_visible_but_clipped_or_overlapping_targets_are_rejected(self):
        original = Observation()
        for field, value in (("rect", [460, 6, 486, 32]),
                             ("rect", [8, 100, 34, 126]),
                             ("window", 8)):
            c = copy.deepcopy(original)
            c.nodes["scene/views"][field] = value
            with self.assertRaises(AssertionError):
                toolbar_geometry(c, "scene/")

    def test_detached_stale_or_offscreen_menu_is_rejected(self):
        original = Observation()
        for field, value in (("parent", 2), ("window", 8),
                             ("rect", [8, 200, 200, 260]),
                             ("rect", [400, 36, 600, 100])):
            c = copy.deepcopy(original)
            c.nodes["dock-menu"][field] = value
            with self.assertRaises(AssertionError):
                anchored_menu(c, "scene/", "views")

    def test_clipped_navigation_and_wrong_orientation_size_are_rejected(self):
        for name, rect in [("pan", [442, 350, 468, 376]),
                           ("orbit", [372, 46, 468, 120])]:
            c = Observation()
            c.nodes["scene/" + name]["rect"] = rect
            with self.assertRaises(AssertionError):
                toolbar_geometry(c, "scene/")

    def test_short_canvas_must_not_retain_visible_navigation_targets(self):
        c = Observation()
        c.nodes["scene/viewport"]["rect"][3] = 220
        with self.assertRaises(AssertionError):
            toolbar_geometry(c, "scene/")


if __name__ == "__main__":
    unittest.main()
