import os
import tempfile
import unittest

from gui_gate import sidefx


class SideEffectTests(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        os.makedirs(f"{self.d}/dir1")
        os.makedirs(f"{self.d}/dir2")
        open(f"{self.d}/file1", "w").write("new\n")
        open(f"{self.d}/dir2/file1", "w").write("keep me\n")
        os.makedirs(f"{self.d}/src/a")
        open(f"{self.d}/src/a/x.ipynb", "w").write("{}")
        os.makedirs(f"{self.d}/out/src/a")
        open(f"{self.d}/out/src/a/x.ipynb", "w").write("old")

    def eff(self, cmd):
        return sidefx.shell_effects(cmd, self.d)

    def test_cp_into_dir_with_existing_file(self):
        self.assertIsNone(self.eff("cp file1 dir1"))
        r = self.eff("cp file1 dir1 && cp file1 dir2")
        self.assertEqual(r[0], "high")
        self.assertIn("dir2/file1", r[1])
        self.assertIsNone(self.eff("cp -n file1 dir2"))
        self.assertEqual(self.eff("for d in dir1 dir2; do cp file1 $d; done") , None)  # loops are not parsed

    def test_redirect_overwrite(self):
        self.assertEqual(self.eff("echo hi > dir2/file1")[0], "high")
        self.assertIsNone(self.eff("echo hi >> dir2/file1"))
        self.assertIsNone(self.eff("echo hi > newfile"))

    def test_recursive_copy_and_parents(self):
        self.assertEqual(self.eff("cp -r src out")[0], "high")
        self.assertEqual(self.eff("cp --parents src/a/x.ipynb out")[0], "high")
        self.assertEqual(self.eff("find src -name '*.ipynb' -exec cp --parents {} out \\;")[0], "high")
        self.assertIsNone(self.eff("find src -name '*.ipynb' -exec cp --parents {} dir1 \\;"))

    def test_chmod(self):
        self.assertEqual(self.eff("chmod -R 644 .")[0], "high")
        self.assertEqual(self.eff("chmod -R 755 .")[0], "medium")
        self.assertIsNone(self.eff("find . -type f -exec chmod 644 {} \\;"))
        self.assertIsNone(self.eff("chmod 644 file1"))

    def test_rm(self):
        self.assertEqual(self.eff("rm file1")[0], "medium")
        self.assertEqual(self.eff("rm -rf dir2")[0], "high")
        self.assertIsNone(self.eff("rm nothere"))

    def test_overwrite_dialog(self):
        blob = 'A file named "pre.pptx" already exists. Do you want to overwrite it? Yes No'
        self.assertTrue(sidefx.overwrite_dialog(blob, "Yes", set()))
        self.assertFalse(sidefx.overwrite_dialog(blob, "No", set()))
        self.assertTrue(sidefx.overwrite_dialog(blob, "", {"return"}))
        self.assertFalse(sidefx.overwrite_dialog("Save Document? Save Don't Save", "Save", set()))


if __name__ == "__main__":
    unittest.main()
