"""Run the existing compositor conflict/rollback contract against the native template."""
import importlib.machinery
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module

tests = load('legacy_shortcuts_tests', ROOT / 'tests/omarchy_shortcuts_test.py')
tests.integration = load('native_shortcuts', ROOT / 'omarchy/scripts/shortcuts.py')
tests.TEMPLATE = (ROOT / 'omarchy/shortcuts.lua').read_text()
suite = unittest.defaultTestLoader.loadTestsFromTestCase(tests.Shortcuts)
result = unittest.TextTestRunner(verbosity=1).run(suite)
raise SystemExit(0 if result.wasSuccessful() else 1)
