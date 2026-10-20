"""Owner-reviewed pixel exceptions, never inferred from a failing diff or live bounds."""
import copy
import json
from pathlib import Path

PATH = Path(__file__).resolve().parents[2] / "docs/ui/docking/accepted-variance.json"


def validate_text(scenario, elements):
    """Fixed semantic fixture text remains required despite pixel-style exceptions."""
    policy = json.loads(PATH.read_text())
    # The exception rectangles cover only these authored caption identities.
    required = policy["required_text"].get(scenario, {})
    for identity, text in required.items():
        matches = [e for e in elements if e["id"] == identity and e["visible"]]
        if len(matches) != 1 or matches[0].get("text") != text:
            raise ValueError(f"Accepted text variance requires the original visible caption: {identity}")


def accepted(config, revision):
    policy = json.loads(PATH.read_text())
    if (policy["schema"] != 1 or policy["fixture"] != config["fixture"]
            or policy["reference_revision"] != revision
            or policy["approval"] != "human_reviewed"):
        raise ValueError("Accepted visual policy does not match reviewed references")
    # Required key: an unknown/new scenario cannot silently inherit exemptions.
    rectangles = policy["text_interiors"][config["scenario"]]
    result = copy.deepcopy(config)
    result["reference"]["approval"] = "human_reviewed"
    result["comparison"]["max_excluded_ppm"] = policy["max_excluded_ppm"]
    result["exclusions"] = [
        {"name": f"approved-text-{index}", "reason": policy["reason"],
         "kind": "approved_visual_variance", "rect": rect}
        for index, rect in enumerate(rectangles)
    ]
    return result
