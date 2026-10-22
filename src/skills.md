# gui-gate built-in skills (0.9.7, GUI_GATE_SKILLS). One section per app; the gate picks sections by the task's words
# and the open windows and puts them in the task briefing (Intelligence-Indeed static skills, Pointer knowledge bank).
# No task answers here: never write what a specific benchmark task needs, only how the app works.

## environment
keys: *
Graders check the real end state: files on disk, settings, app state - not how the screen looks. Save explicitly (ctrl+s / File > Save) unless the task says otherwise; when an evaluator saves the open document itself it does so in the open window, so the window must hold the final content. When a task gives an input file and asks for a new file without a full path, put it next to the input. Operate on the exact file / app the task names (a pre-opened or pre-downloaded file is the one to edit in place). A file you change in code while it is open in an app: close or reload it in the app, or the app's copy wins. Use a real feature when the task names one (a pivot table, a chart, a style, a setting), not a lookalike.

## libreoffice_calc
keys: calc, spreadsheet, xlsx, ods, csv, cell, column, row, sheet, formula, pivot, chart
LibreOffice Calc: jump to a cell or range with the Name Box (ctrl+shift+t or the box left of the formula bar), type A1 or B2:B20, Enter. ctrl+Home = A1, ctrl+End = last used cell, ctrl+arrow = edge of the data. Type a value or a formula (=A2*2) and Enter (down) or Tab (right); fill down: select the range, ctrl+d. Format > Cells = ctrl+1 (number format, font, borders); Data > Sort, Data > AutoFilter, Data > Pivot Table (alt+d) via the menu bar. Percent as a ratio with % format (0.075 -> 7.5%) unless the header says the column holds percent points. Named colours: Red #FF0000, Green #00A933, Blue #0000FF, Yellow #FFFF00, Dark Red #800000, Dark Green #008000. Pivot tables and charts must be real objects made through Insert/Data menus. Bulk computation (>20 cells) may be done in code (python3 + openpyxl when installed), then reload the file in Calc (File > Reload, do not save first).

## libreoffice_writer
keys: writer, docx, odt, document, paragraph, page, font, heading, footnote, table of contents, word
LibreOffice Writer: ctrl+Home / ctrl+End = start / end; ctrl+shift+down selects to the end of the paragraph; four clicks select a paragraph, three a sentence. Format > Paragraph (indents, line spacing), Format > Character (font, strikethrough, colour, super/subscript: ctrl+shift+p / ctrl+shift+b), Format > Page Style (margins, columns, header/footer). Insert > Page Break = ctrl+Enter; Insert > Field > Page Number. Styles: F11 (Styles sidebar). Tools > Options > LibreOffice Writer holds defaults. Never edit the .docx XML to change formatting.

## libreoffice_impress
keys: impress, pptx, odp, slide, presentation, master, notes
LibreOffice Impress: page down / page up move between slides in Normal view; Tab cycles the objects of a slide, Enter (or F2) edits the selected text box, Escape leaves it. Slide properties / background: Slide > Slide Properties; master slides: View > Master Slide (a presentation can have more than one master - check them all). Format > Character / Paragraph for text; Insert > Table. File > Save As (ctrl+shift+s) and File > Export (PNG/PDF) open a dialog: type the full path in the file name, check the file type, Save; keep the format when asked.

## chrome
keys: chrome, browser, tab, bookmark, website, url, search engine, cookies, extension, homepage
Chrome: settings pages open from the address bar (ctrl+l, type, Enter): chrome://settings/fonts, /languages, /appearance, /searchEngines, /cookies, /privacy, /onStartup, chrome://extensions, chrome://bookmarks, chrome://history. ctrl+f finds text on a page; ctrl+d bookmarks the page; ctrl+shift+delete clears browsing data (use that dialog, not code). Do interactions in the GUI (address bar, clicks, forms) - graders check the URLs and browser state that produces. Open new pages in a new tab (ctrl+t) and keep existing tabs. inspect kind=page reads tabs and page text when Chrome has a debugging port.

## vlc
keys: vlc, video, audio, media, playlist, subtitle, playback
VLC: Tools > Preferences = ctrl+p; "Show settings: All" (bottom left) shows every option; Interface holds the look, Hotkeys holds shortcuts (global hotkeys work without focus). Changes need Save in the preferences dialog; some need a VLC restart. Media > Convert/Save for conversions; Video > Take Snapshot.

## gimp
keys: gimp, image, photo, png, jpg, jpeg, layer, canvas, brightness, contrast, saturation, crop, resize
GIMP: Image > Mode holds the image colour mode; Image > Scale Image / Canvas Size; Colors menu holds brightness-contrast, hue-saturation, curves; Layer menu and the Layers dock (ctrl+l) for layers. File > Export As (ctrl+shift+e) writes PNG/JPG (pick the type by the file name's extension); File > Overwrite writes over the original. Changes visible only on the canvas do not count until exported.

## thunderbird
keys: thunderbird, email, mail, inbox, message, filter, folder, account
Thunderbird: Message Filters under Tools (or the app menu) > Message Filters; folders: right-click a folder > New Folder; star / tag / move messages in the GUI (it updates Thunderbird's own databases). Account settings: Edit > Account Settings (or the app menu).

## vs_code
keys: vs code, vscode, visual studio code, editor, extension, settings.json, keybinding
VS Code: ctrl+shift+p command palette; ctrl+, settings (search the setting name); ctrl+k ctrl+s keyboard shortcuts; ctrl+shift+x extensions; ctrl+` terminal. Settings changed in the Settings UI are written to settings.json.

## os
keys: terminal, folder, directory, file, permission, chmod, copy, move, rename, delete, zip, tar, install, clone, git, user, timezone, wallpaper
Terminal and files: the working directory a task means is the one its terminal shows. Look before you change: list what a command would touch (ls, find ... -print) before cp/mv/chmod/rm. Keep existing files unless the task names them: cp -n or a new name. Desktop settings: gsettings / xfconf-query or the Settings app. run_command runs bash and returns the output; long work: run_command mode=start, then mode=poll.

## multi_apps
keys: (picked when the task involves two or more apps)
Several apps: list_windows shows every window; bring_to_front a window before typing into it. Read data out of a file with inspect kind=file or run_command, then do the writing in the target app; copy between apps with ctrl+c / ctrl+v.
