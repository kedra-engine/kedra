"""Boundary tests for native evidence validation, not a substitute UI renderer."""
import unittest
from window_chrome import validate_row


class HeaderEvidenceTests(unittest.TestCase):
    def row(self):
        return {"id": "window.chrome", "window": 7, "visible": True,
                "rect": [1, 1, 799, 33]}

    def test_rejects_stacked_or_missing_header_geometry(self):
        row = self.row()
        row["rect"][3] = 65
        with self.assertRaises(AssertionError):
            validate_row(row, 7)
        row["rect"] = [0, 0, 0, 32]
        with self.assertRaises(AssertionError):
            validate_row(row, 7)

    def test_rejects_hidden_foreign_or_nonfinite_evidence(self):
        for field, value in [("visible", False), ("window", 8),
                             ("rect", [0, 0, float("nan"), 32])]:
            row = self.row()
            row[field] = value
            with self.assertRaises(AssertionError):
                validate_row(row, 7)

    def test_accepts_native_header_in_logical_coordinates(self):
        validate_row(self.row(), 7)


if __name__ == "__main__":
    unittest.main()
