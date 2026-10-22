# Per-app keyboard hints for the bench prompt (static text; bench/app_hints.py picks sections by task name)

## libreoffice_calc
LibreOffice Calc: jump to a cell or range by typing it in the Name Box (ctrl+shift+t, or click the box left of the
formula bar), then Enter. Ctrl+Home = A1, ctrl+End = last used cell, ctrl+arrow = edge of the data. Type a value or a
formula (=A2*2) and press Enter (down) or Tab (right); fill a formula down: select the range (shift+click or Name Box
"B2:B20"), then ctrl+d. Format > Cells is ctrl+1; Data > Pivot Table via the menu bar (alt+d). The open document is
saved by the evaluation itself, in the open window - if you change the .xlsx/.ods file in code, reload it in
LibreOffice (File > Reload, without saving) so the window shows your change.

## libreoffice_writer
LibreOffice Writer: ctrl+Home / ctrl+End = start / end of the document; ctrl+shift+down selects to the end of the
paragraph; four clicks select a whole paragraph (three select a sentence). Format > Paragraph (alt+o, then a) holds
line spacing; Format > Character holds strikethrough / underline / colour. Insert > Page Break is ctrl+Enter.
Insert > Header and Footer, then Insert > Field > Page Number for page numbers. The open document is saved by the
evaluation in the open window; save under a new name only when the task asks for a file.

## libreoffice_impress
LibreOffice Impress: the slide pane lists slides; page down / page up move between slides in the normal view. Tab
cycles through the objects of the current slide, Enter (or F2) edits the selected text box, Escape leaves it.
Format > Character / Paragraph for text; Insert > Table for tables. File > Save As (ctrl+shift+s) and File > Export
(PNG / PDF) open a dialog: type the full path in the file name field, check the file type, then Save.

## chrome
Chrome: settings pages open directly in the address bar (ctrl+l, type, Enter): chrome://settings/fonts,
chrome://settings/languages, chrome://settings/appearance, chrome://settings/searchEngines; ctrl+f finds text on a page.

## vlc
VLC: Tools > Preferences is ctrl+p; "Show settings: All" (bottom left) shows every option; Interface settings hold
the look (skins / colours). Changes need Save in the preferences dialog.

## gimp
GIMP: Image > Mode holds the image's colour mode. Colours, layers and filters are in
the menu bar; File > Export As (ctrl+shift+e) writes PNG/JPG, File > Overwrite writes over the original.

## os
Terminal and files: the working directory a task means is the one its terminal shows. Look before you change: list what a command would touch (ls, find ... -print)
before cp/mv/chmod/rm. Keep existing files: use cp -n / a new name rather than overwriting.

## multi_apps
Several apps: list_windows shows every window; bring_to_front a window before typing into it. Copy data between
apps with ctrl+c / ctrl+v.
