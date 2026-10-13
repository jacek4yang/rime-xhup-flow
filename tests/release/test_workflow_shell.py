"""Embedded qualification shell is validated without executing it."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest

from check_workflow_shell import check_qualification_commands


class QualificationShellTests(unittest.TestCase):
    def test_unclosed_shell_quote_in_valid_python_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'qualification.py'
            command = 'xhup-cli doctor --user-data-dir "$package'
            source.write_text(f'step("native-controls", {json.dumps(command)})\n')
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(check_qualification_commands(str(source)), 1)

    def test_valid_command_is_parsed_without_side_effects(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'qualification.py'
            marker = Path(directory) / 'must-not-exist'
            command = f'touch "{marker}"\nxhup-cli doctor --user-data-dir "$package"\n'
            source.write_text(f'step("native-controls", {json.dumps(command)})\n')
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(check_qualification_commands(str(source)), 0)
            self.assertFalse(marker.exists())


if __name__ == '__main__':
    unittest.main()
