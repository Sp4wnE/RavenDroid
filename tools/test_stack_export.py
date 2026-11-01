"""Tests for the pinned transform; no game data needed."""
import hashlib, unittest
from unittest.mock import patch
import prepare_stack_export as module
class StackExportTests(unittest.TestCase):
    def fixture(self, save=bytes.fromhex("0023070b")):
        def section(kind, payload): return bytes([kind])+module.leb(len(payload))+payload
        exports=module.leb(2)
        for name, index in [(b"stackSave",822),(b"stackRestore",823)]:
            exports+=module.leb(len(name))+name+bytes([0])+module.leb(index)
        code=module.leb(4)
        for body in [b"\x00\x0b",b"\x00\x0b",save,bytes.fromhex("00200024070b")]:code+=module.leb(len(body))+body
        return b"\x00asm\x01\x00\x00\x00"+section(7,exports)+section(10,code)+section(0,b"\x03tagunchanged")
    def test_unknown_source_rejected(self):
        with self.assertRaises(ValueError):module.transform(self.fixture())
    def test_only_export_section_changes(self):
        source=self.fixture()
        with patch.object(module,'SOURCE_SHA',hashlib.sha256(source).hexdigest()):result=module.transform(source)
        self.assertIn(module.EXPORT,result)
        # Every code/custom-section byte is identical; helper bodies are untouched.
        source_tail=source.index(bytes([10]),8);result_tail=result.index(bytes([10]),source_tail+27)
        self.assertEqual(source[source_tail:],result[result_tail:])
        self.assertEqual(len(result)-len(source),27)
    def test_non_pure_helper_rejected_even_with_expected_hash(self):
        source=self.fixture(bytes.fromhex("0041070b"))
        with patch.object(module,'SOURCE_SHA',hashlib.sha256(source).hexdigest()):
            with self.assertRaises(ValueError):module.transform(source)
if __name__=='__main__':unittest.main()
