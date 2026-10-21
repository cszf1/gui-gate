import os, tempfile, unittest
from gui_gate import sidefx

class LOExportDialog(unittest.TestCase):
    def test_libreoffice_export_dialog_existing_pdf(self):
        txt = open(os.path.join(os.path.dirname(__file__), "fixtures_og02_export.txt")).read()
        with tempfile.TemporaryDirectory() as d:
            txt = txt.replace("/home/user/Desktop", d)
            self.assertIsNone(sidefx.save_dialog_target(txt))
            with open(os.path.join(d, "View_Person_Organizational_Summary.pdf"), "w") as f:
                f.write("%PDF-1.4 x")
            self.assertTrue(sidefx.save_dialog_target(txt).endswith(".pdf"))

if __name__ == "__main__":
    unittest.main()
