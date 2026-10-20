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


class MenuRiskTests(unittest.TestCase):
    def risk(self, role, label):
        from gui_gate.features import Target, assess_risk
        from gui_gate.state import Element, GateState
        t = Target(kind="token", pid=999999, element=Element(1, role, label))
        return assess_risk("click", {}, t, [], GateState())[:2]

    def test_menu_openers_are_not_risky(self):
        self.assertEqual(self.risk("menu", "Format"), (None, None))
        self.assertEqual(self.risk("menu", "Edit"), (None, None))
        self.assertEqual(self.risk("button", "Format"), (None, None))
        self.assertEqual(self.risk("menu item", "Delete Rows")[1], "medium")
        self.assertEqual(self.risk("button", "Delete")[1], "high")
        self.assertEqual(self.risk("button", "Format Disk")[1], "high")


class OverwriteDialogTests(unittest.TestCase):
    def test_yes_in_replace_dialog_is_high(self):
        from gui_gate.features import Target, assess_risk
        from gui_gate.state import GateState
        st = GateState()
        md = ('- alert = "Question"\n  - label = "A file named "pre.pptx" already exists.\n\nDo you want to replace it?"\n'
              '  - [0] button "No" [actions=[click]]\n  - [1] button "Yes" [actions=[click]]')
        res = {"structuredContent": {"snapshot_id": "s00000009", "pid": 4242, "window_id": 7, "tree_markdown": md},
               "content": [{"type": "text", "text": md}]}
        o = st.record_observation({"pid": 4242, "window_id": 7}, res)
        yes = o.elements.get(1)
        self.assertIsNotNone(yes)
        t = Target(kind="token", pid=4242, window_id=7, snapshot="s00000009", index=1, element=yes, obs=o)
        self.assertEqual(assess_risk("click", {}, t, [], st)[1], "high")
        no = Target(kind="token", pid=4242, window_id=7, snapshot="s00000009", index=0, element=o.elements.get(0), obs=o)
        self.assertIsNone(assess_risk("click", {}, no, [], st)[1])
        key = Target(kind="keyboard", pid=4242)
        self.assertEqual(assess_risk("press_key", {"key": "Return"}, key, [], st)[1], "high")


class FastPathTests(unittest.TestCase):
    def test_nearest_and_opener(self):
        from gui_gate.server import _nearest, _fast_path
        from gui_gate.state import Element
        from gui_gate.features import Target, ActionCtx
        old = Element(5, "menu", "Insert", {"x": 100, "y": 10, "w": 40, "h": 20})
        a = Element(7, "menu", "Insert", {"x": 101, "y": 10, "w": 40, "h": 20})
        b = Element(90, "menu", "Insert", {"x": 600, "y": 300, "w": 40, "h": 20})
        self.assertEqual(_nearest(old, [b, a]), [a])
        self.assertEqual(len(_nearest(Element(5, "menu", "Insert"), [Element(6, "menu", "Insert"), Element(7, "menu", "Insert")])), 2)
        self.assertEqual(_nearest(Element(5, "menu", "Insert"), [Element(6, "menu", "Insert"), Element(70, "menu", "Insert")])[0].index, 6)
        t = Target(kind="token", pid=1, element=old)
        ctx = ActionCtx.__new__(ActionCtx)
        ctx.target, ctx.tool = t, "click"
        self.assertIsNotNone(_fast_path(ctx, {}))
        ctx.target = Target(kind="pixel", pid=1)
        self.assertIsNotNone(_fast_path(ctx, {"obs_had_shot": 1.0, "acts_since_obs": 0.0}))
        self.assertIsNone(_fast_path(ctx, {"obs_had_shot": 0.0, "acts_since_obs": 0.0}))


class ScreenshotOnlyReadTests(unittest.TestCase):
    def test_screenshot_only_read_counts_as_observation(self):
        from gui_gate.state import GateState
        st = GateState()
        res = {"structuredContent": {"pid": 77, "screenshot_width": 800, "screenshot_height": 600},
               "content": [{"type": "image", "data": "x", "mimeType": "image/png"}]}
        o = st.record_observation({"pid": 77, "include_screenshot": True}, res)
        self.assertIsNotNone(o)
        self.assertTrue(o.with_screenshot)
        self.assertIs(st.latest_for(77, 5), o)
