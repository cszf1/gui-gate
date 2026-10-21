# Per-turn time breakdown (bench/turn_breakdown.py)

## og01-impress-pre-pptx-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.6 | 3.1 | list_apps 0.05, list_windows 0.06 | 85+4224 | 58 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.5 | get_window_state 0.38 | 1809+4224 | 146 | 31 | 125 | 0 |  |
| 3 | 39.3 | 40.5 | press_key 0.09 | 4899+6016 | 210 | 89 | 369 | 1 | 550 KB image/png 1396x823 |
| 4 | 51.5 | 51.9 | list_windows 0.00 | 288+10880 | 52 | 7 | 31 | 1 |  |
| 5 | 50.3 | 50.7 | get_window_state 0.26 | 172+11136 | 92 | 0 | 0 | 1 |  |
| 6 | 56.0 | 59.9 | click 1.46 | 1670+11264 | 553 | 482 | 1975 | 2 | 75 KB image/png 782x497 |
| 7 | 36.4 | 36.8 | press_key 0.11 | 626+12928 | 84 | 0 | 0 | 2 |  |
| 8 | 35.3 | 35.9 | press_key 0.89 | 223+13440 | 124 | 26 | 108 | 2 |  |
| 9 | 60.6 | 61.0 | type_text 1.06 | 262+13568 | 83 | 0 | 0 | 2 |  |
| 10 | 30.2 | 30.8 | press_key 0.86 | 133+13824 | 79 | 0 | 0 | 2 |  |
| 11 | 37.1 | 37.4 | get_window_state 0.51 | 127+13952 | 100 | 0 | 0 | 2 |  |
| 12 | 57.6 | 59.6 | click 1.47 | 1899+13952 | 339 | 259 | 1071 | 3 | 78 KB image/png 782x497 |
| 13 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 470.4 s, tool total 7.2 s over 13 turns; gate: 7 actions exec 5.8 s, gate decision 0.09 s, 5 observations 0.8 s (3 with screenshot), reviewer calls 0 (0.0 s)

## og01-impress-pre-pptx-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.5 | list_windows 0.05 | 85+4224 | 36 | 8 | 32 | 0 |  |
| 2 | 1.8 | 2.4 | get_window_state 0.36 | 314+4224 | 110 | 28 | 100 | 0 |  |
| 3 | 14.2 | 14.7 | press_key 0.75 | 3003+4480 | 125 | 24 | 99 | 1 | 550 KB image/png 1396x823 |
| 4 | 47.1 | 48.6 | run_actions 2.37 | 1497+7424 | 292 | 150 | 609 | 1 |  |
| 5 | 46.5 | 47.1 | get_window_state 0.01 | 531+8832 | 82 | 0 | 0 | 1 |  |
| 6 | 45.3 | 45.6 | list_windows 0.01 | 137+9344 | 38 | 10 | 44 | 1 |  |
| 7 | 52.8 | 55.3 | get_window_state 0.24 | 241+9472 | 354 | 272 | 1169 | 1 |  |
| 8 | 82.1 | 87.6 | press_key 0.69 | 2541+9600 | 792 | 691 | 3024 | 2 | 550 KB image/png 1396x823 |
| 9 | 93.6 | 96.0 | run_actions 5.22 | 3325+12032 | 349 | 179 | 714 | 2 |  |
| 10 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 399.9 s, tool total 9.7 s over 10 turns; gate: 9 actions exec 7.3 s, gate decision 0.04 s, 8 observations 2.0 s (2 with screenshot), reviewer calls 0 (0.0 s)

## og02-writer-pdf-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.8 | 3.1 | list_windows 0.02, list_apps 0.07 | 83+4224 | 58 | 12 | 46 | 0 |  |
| 2 | 2.4 | 3.0 | get_window_state 0.29 | 1808+4224 | 100 | 36 | 151 | 0 |  |
| 3 | 10.9 | 11.9 | click 0.16 | 4583+6016 | 181 | 133 | 577 | 1 | 313 KB image/png 1396x823 |
| 4 | 5.3 | 5.6 | get_window_state 0.28 | 371+10496 | 64 | 0 | 0 | 1 |  |
| 5 | 29.7 | 31.0 | click 0.89 | 1767+10752 | 158 | 110 | 471 | 2 | 67 KB image/png 782x444 |
| 6 | 31.6 | 32.5 | get_window_state 2.06 | 326+12416 | 64 | 0 | 0 | 2 |  |
| 7 | 30.6 | 30.6 | get_window_state 1.06 | 554+12672 | 105 | 24 | 100 | 3 | 17 KB image/png 546x115 |
| 8 | 20.1 | 22.3 | click 5.15 | 542+13184 | 224 | 130 | 497 | 4 | 17 KB image/png 546x115 |
| 9 | 3.8 | 4.2 | list_windows 0.01 | 323+13696 | 45 | 0 | 0 | 4 |  |
| 10 | 16.6 | 17.3 | get_window_state 0.06 | 221+13952 | 132 | 49 | 221 | 4 |  |
| 11 | 23.8 | 26.3 | click 0.71 | 503+14080 | 376 | 282 | 1028 | 5 | 17 KB image/png 546x115 |
| 12 | 36.5 | 36.9 | get_window_state 0.01 | 564+14464 | 83 | 0 | 0 | 5 |  |
| 13 | 41.7 | 42.2 | list_windows 0.05 | 171+14976 | 60 | 15 | 80 | 5 |  |
| 14 | 27.5 | 28.8 | get_window_state 0.71 | 162+15104 | 219 | 137 | 594 | 5 |  |
| 15 | 41.3 | 43.9 | click 1.73 | 1047+15232 | 348 | 238 | 1074 | 6 | 45 KB image/png 1356x848 |
| 16 | 34.0 | 34.6 | get_window_state 0.48 | 440+16256 | 82 | 0 | 0 | 6 |  |
| 17 | 30.7 | 31.4 | get_window_state 0.14 | 222+16640 | 105 | 22 | 122 | 6 |  |
| 18 | 34.6 | 36.9 | press_key 0.89 | 951+16768 | 344 | 246 | 1061 | 7 | 45 KB image/png 1356x848 |
| 19 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 442.6 s, tool total 14.8 s over 19 turns; gate: 6 actions exec 9.5 s, gate decision 0.04 s, 12 observations 4.7 s (7 with screenshot), reviewer calls 0 (0.0 s)

## og02-writer-pdf-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.5 | 3.9 | list_windows 0.01 | 83+4224 | 38 | 0 | 0 | 0 |  |
| 2 | 3.1 | 3.7 | get_window_state 0.32 | 312+4224 | 100 | 36 | 139 | 0 |  |
| 3 | 19.7 | 20.5 | click 0.62 | 4623+4480 | 132 | 58 | 265 | 1 | 313 KB image/png 1396x823 |
| 4 | 14.2 | 15.4 | click 0.02 | 1576+9088 | 205 | 125 | 565 | 1 |  |
| 5 | 12.5 | 14.2 | click 2.56 | 371+10624 | 208 | 139 | 592 | 1 |  |
| 6 | 29.8 | 30.1 | press_key 0.92 | 1712+10880 | 99 | 15 | 55 | 1 |  |
| 7 | 28.4 | 29.5 | click 4.63 | 1495+12544 | 85 | 21 | 85 | 1 |  |
| 8 | 32.0 | 33.7 | click 1.26 | 1569+13952 | 202 | 154 | 635 | 1 |  |
| 9 | 9.0 | 11.3 | type_text 4.83 | 1632+15488 | 315 | 249 | 1140 | 1 |  |
| 10 | 9.0 | 13.3 | press_key 1.18 | 1810+17024 | 506 | 408 | 1713 | 1 |  |
| 11 | 23.8 | 24.3 | press_key 1.29 | 1892+18816 | 156 | 77 | 290 | 1 |  |
| 12 | 17.1 | 17.6 | type_text 0.90 | 1624+20608 | 91 | 0 | 0 | 1 |  |
| 13 | 20.6 | 21.2 | get_window_state 0.25 | 223+22144 | 116 | 17 | 75 | 1 |  |
| 14 | 24.3 | 25.4 | get_window_state 0.21 | 439+22272 | 163 | 63 | 269 | 1 |  |
| 15 | 8.4 | 9.6 | click 0.94 | 562+22656 | 188 | 139 | 548 | 1 |  |
| 16 | 19.5 | 21.1 | press_key 0.65 | 311+23168 | 271 | 173 | 751 | 1 |  |
| 17 | 37.8 | 38.7 | press_key 1.34 | 1404+23424 | 284 | 186 | 794 | 1 |  |
| 18 | 28.8 | 30.2 | type_text 1.05 | 1463+24704 | 213 | 129 | 588 | 1 |  |
| 19 | 31.3 | 31.7 | press_key 1.24 | 312+26112 | 79 | 0 | 0 | 1 |  |
| 20 | 7.8 | 9.1 | click 0.88 | 1804+26368 | 107 | 48 | 165 | 1 |  |
| 21 | 15.2 | 16.0 |  | 192+28160 | 108 | 0 | 0 | 1 |  |

model total 420.4 s, tool total 25.1 s over 21 turns; gate: 16 actions exec 9.4 s, gate decision 10.13 s, 15 observations 5.1 s (1 with screenshot), reviewer calls 3 (8.8 s)

## ow01-libreoffice_calc-7efeb4b1-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.5 | 1.7 | list_windows 0.01 | 105+4224 | 38 | 0 | 0 | 0 |  |
| 2 | 3.0 | 3.3 | get_window_state 0.31 | 334+4224 | 72 | 8 | 30 | 0 |  |
| 3 | 27.4 | 28.7 | click 1.33 | 4130+4480 | 370 | 276 | 980 | 1 | 381 KB image/png 1396x823 |
| 4 | 35.9 | 36.3 | press_key 0.88 | 473+8576 | 103 | 5 | 13 | 1 |  |
| 5 | 27.6 | 28.7 | get_window_state 0.16 | 235+8960 | 83 | 0 | 0 | 1 |  |
| 6 | 68.8 | 70.9 | click 1.86 | 939+9088 | 385 | 291 | 945 | 2 | 377 KB image/png 1396x823 |
| 7 | 26.2 | 26.9 | type_text 0.93 | 497+9984 | 82 | 0 | 0 | 2 |  |
| 8 | 80.0 | 80.7 | press_key 0.88 | 239+10368 | 79 | 0 | 0 | 2 |  |
| 9 | 13.6 | 14.4 | type_text 1.02 | 233+10496 | 147 | 60 | 212 | 2 |  |
| 10 | 30.1 | 30.6 | press_key 0.88 | 296+10624 | 98 | 0 | 0 | 2 |  |
| 11 | 44.1 | 44.5 | get_window_state 0.19 | 181+10880 | 83 | 0 | 0 | 2 |  |
| 12 | 60.7 | 62.8 | press_key 0.10 | 938+11008 | 421 | 323 | 1179 | 3 | 430 KB image/png 1396x823 |
| 13 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 429.5 s, tool total 8.5 s over 13 turns; gate: 8 actions exec 7.8 s, gate decision 0.08 s, 4 observations 0.4 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.1 | list_windows 0.03 | 105+4224 | 41 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.2 | get_window_state 0.30 | 337+4224 | 74 | 10 | 42 | 0 |  |
| 3 | 20.8 | 23.0 | press_key 0.44 | 4147+4480 | 437 | 339 | 1153 | 1 | 381 KB image/png 1396x823 |
| 4 | 26.7 | 27.2 | get_window_state 0.15 | 3269+8576 | 113 | 12 | 53 | 1 |  |
| 5 | 24.0 | 27.6 | run_actions 5.45 | 931+11776 | 569 | 382 | 1343 | 2 | 377 KB image/png 1396x823 |
| 6 | 21.7 | 23.5 | get_window_state 0.38 | 3606+12672 | 136 | 35 | 144 | 2 |  |
| 7 | 40.9 | 42.5 | press_key 0.74 | 960+16256 | 285 | 206 | 753 | 3 | 439 KB image/png 1396x823 |
| 8 | 59.3 | 59.6 | run_actions 1.00 | 3142+17152 | 150 | 45 | 172 | 3 |  |
| 9 | 53.1 | 54.0 | get_window_state 0.15 | 3038+20224 | 101 | 0 | 0 | 3 |  |
| 10 | 140.6 | 141.0 | press_key 1.76 | 997+23168 | 174 | 76 | 253 | 4 | 318 KB image/png 1396x823 |
| 11 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 402.7 s, tool total 10.4 s over 11 turns; gate: 9 actions exec 6.4 s, gate decision 0.04 s, 10 observations 3.5 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow02-libreoffice_writer-0810415c-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.1 | list_windows 0.06 | 84+4224 | 41 | 0 | 0 | 0 |  |
| 2 | 1.4 | 1.8 | get_window_state 0.32 | 314+4224 | 80 | 16 | 62 | 0 |  |
| 3 | 5.0 | 8.6 | run_actions 1.24 | 4577+4480 | 639 | 512 | 2280 | 1 | 389 KB image/png 1396x823 |
| 4 | 20.9 | 25.4 | click 0.44 | 3780+8960 | 536 | 423 | 1538 | 1 |  |
| 5 | 27.1 | 29.9 | click 1.48 | 4341+12672 | 394 | 281 | 1216 | 2 | 389 KB image/png 1396x823 |
| 6 | 37.0 | 37.7 | get_window_state 0.15 | 580+16896 | 116 | 33 | 110 | 2 |  |
| 7 | 46.0 | 47.1 | click 0.85 | 933+17408 | 149 | 48 | 206 | 3 | 402 KB image/png 1396x823 |
| 8 | 44.0 | 44.3 | get_window_state 0.29 | 273+18304 | 64 | 0 | 0 | 3 |  |
| 9 | 49.5 | 50.0 | click 1.10 | 5413+18560 | 90 | 25 | 76 | 4 | 55 KB image/png 311x692 |
| 10 | 45.8 | 47.3 | list_windows 0.05 | 236+23936 | 185 | 140 | 599 | 4 |  |
| 11 | 34.9 | 39.0 | run_actions 0.84 | 350+24064 | 481 | 352 | 1608 | 4 |  |
| 12 | 97.0 | 99.0 | list_windows 0.01 | 695+24320 | 180 | 152 | 691 | 4 |  |
| 13 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 432.2 s, tool total 6.8 s over 13 turns; gate: 7 actions exec 5.2 s, gate decision 0.03 s, 8 observations 1.3 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow02-libreoffice_writer-0810415c-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.3 | 3.6 | list_apps 0.08, list_windows 0.08 | 84+4224 | 56 | 0 | 0 | 0 |  |
| 2 | 2.6 | 3.0 | get_window_state 0.63 | 1831+4224 | 132 | 32 | 143 | 0 |  |
| 3 | 18.0 | 22.5 | click 0.63 | 4610+6016 | 706 | 641 | 2861 | 1 | 389 KB image/png 1396x823 |
| 4 | 20.7 | 22.4 | run_actions 1.89 | 3590+10624 | 298 | 161 | 689 | 1 |  |
| 5 | 30.8 | 31.8 | get_window_state 0.14 | 3271+14208 | 146 | 44 | 191 | 1 |  |
| 6 | 5.3 | 7.4 | click 0.36 | 776+17408 | 239 | 174 | 752 | 2 | 297 KB image/png 1200x708 |
| 7 | 52.9 | 54.5 | press_key 0.55 | 3137+18176 | 266 | 168 | 706 | 2 |  |
| 8 | 68.1 | 69.0 | click 0.63 | 2240+21248 | 161 | 96 | 374 | 2 |  |
| 9 | 72.7 | 73.9 | get_window_state 0.39 | 3204+23424 | 181 | 79 | 367 | 2 |  |
| 10 | 98.9 | 102.2 | run_actions 2.47 | 744+26624 | 547 | 371 | 1414 | 3 | 295 KB image/png 1200x708 |
| 11 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 390.3 s, tool total 7.8 s over 11 turns; gate: 9 actions exec 4.0 s, gate decision 0.04 s, 10 observations 2.5 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow03-libreoffice_impress-39be0d19-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 1.9 | list_windows 0.01 | 90+4224 | 39 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.2 | get_window_state 0.30 | 317+4224 | 84 | 20 | 67 | 0 |  |
| 3 | 32.3 | 33.2 | click 0.08 | 4664+4480 | 140 | 75 | 298 | 1 | 311 KB image/png 1396x823 |
| 4 | 25.7 | 26.1 | get_window_state 0.15 | 246+9088 | 83 | 0 | 0 | 1 |  |
| 5 | 28.1 | 28.9 | click 1.89 | 950+9216 | 143 | 49 | 193 | 2 | 311 KB image/png 1396x823 |
| 6 | 20.8 | 21.2 | get_window_state 0.14 | 266+10112 | 83 | 0 | 0 | 2 |  |
| 7 | 77.7 | 77.8 | click 1.44 | 842+10368 | 97 | 32 | 112 | 3 | 247 KB image/png 1396x823 |
| 8 | 53.7 | 53.9 | get_window_state 0.45 | 258+11136 | 64 | 0 | 0 | 3 |  |
| 9 | 83.8 | 84.9 | click 0.02 | 5342+11392 | 65 | 0 | 0 | 4 | 36 KB image/png 279x467 |
| 10 | 73.3 | 74.1 | click 1.49 | 239+16640 | 222 | 128 | 460 | 4 |  |
| 11 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 404.3 s, tool total 6.0 s over 11 turns; gate: 5 actions exec 4.7 s, gate decision 0.14 s, 5 observations 0.6 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow03-libreoffice_impress-39be0d19-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.8 | 3.2 | list_apps 0.05, list_windows 0.05 | 90+4224 | 59 | 0 | 0 | 0 |  |
| 2 | 2.5 | 2.9 | get_window_state 0.32 | 1822+4224 | 85 | 21 | 72 | 0 |  |
| 3 | 6.0 | 6.7 | click 0.44 | 4634+6016 | 131 | 20 | 67 | 1 | 311 KB image/png 1396x823 |
| 4 | 30.5 | 32.9 | get_window_state 0.16 | 2628+10624 | 269 | 168 | 757 | 1 |  |
| 5 | 7.4 | 8.3 | click 2.36 | 1086+13184 | 138 | 27 | 98 | 2 | 311 KB image/png 1396x823 |
| 6 | 69.8 | 72.4 | press_key 1.46 | 3379+14208 | 408 | 310 | 1367 | 3 | 247 KB image/png 1396x823 |
| 7 | 84.9 | 85.5 | click 1.95 | 3613+17536 | 94 | 10 | 42 | 4 | 247 KB image/png 1396x823 |
| 8 | 94.1 | 95.1 | press_key 0.99 | 2589+21120 | 177 | 79 | 359 | 4 |  |
| 9 | 112.3 | 113.1 | click 0.42 | 3148+23680 | 109 | 14 | 50 | 5 | 271 KB image/png 1396x823 |
| 10 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 419.9 s, tool total 8.2 s over 10 turns; gate: 6 actions exec 4.5 s, gate decision 0.04 s, 9 observations 3.1 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow04-chrome-af630914-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.2 | 3.5 | list_apps 0.06 | 109+4224 | 39 | 0 | 0 | 0 |  |
| 2 | 1.6 | 1.9 | list_windows 0.01 | 1782+4224 | 55 | 10 | 40 | 0 |  |
| 3 | 1.6 | 2.0 | get_window_state 0.19 | 223+5888 | 82 | 0 | 0 | 0 |  |
| 4 | 2.1 | 2.9 | click 0.88 | 2157+6016 | 130 | 65 | 283 | 0 |  |
| 5 | 7.3 | 7.4 | type_text 1.10 | 306+8064 | 65 | 0 | 0 | 0 |  |
| 6 | 1.7 | 2.0 | press_key 0.89 | 159+8320 | 60 | 0 | 0 | 0 |  |
| 7 | 2.0 | 2.4 | get_window_state 0.33 | 134+8448 | 82 | 0 | 0 | 0 |  |
| 8 | 3.3 | 6.8 | run_actions 2.95 | 3843+8576 | 519 | 399 | 1693 | 0 |  |
| 9 | 2.9 | 6.1 | click 1.70 | 3220+12416 | 533 | 468 | 1917 | 0 |  |
| 10 | 3.8 | 3.9 | get_window_state 0.28 | 620+15616 | 98 | 0 | 0 | 0 |  |
| 11 | 6.3 | 6.7 | click 0.88 | 755+16128 | 79 | 14 | 67 | 0 |  |
| 12 | 3.1 | 3.2 | get_window_state 1.52 | 272+16768 | 82 | 0 | 0 | 0 |  |
| 13 | 4.4 | 5.6 | press_key 0.87 | 177+17024 | 119 | 59 | 255 | 0 |  |
| 14 | 5.9 | 6.4 | get_window_state 1.06 | 211+17152 | 82 | 0 | 0 | 0 |  |
| 15 | 6.0 | 9.4 | press_key 0.86 | 474+17280 | 461 | 401 | 1628 | 1 | 8 KB image/png 200x146 |
| 16 | 3.0 | 3.6 | get_window_state 1.06 | 594+17664 | 82 | 0 | 0 | 1 |  |
| 17 | 2.5 | 2.8 | press_key 0.10 | 571+18176 | 70 | 10 | 41 | 2 | 8 KB image/png 200x146 |
| 18 | 4.2 | 4.7 | get_window_state 0.41 | 172+18688 | 99 | 0 | 0 | 2 |  |
| 19 | 15.7 | 16.4 | click 1.68 | 1209+18816 | 130 | 65 | 285 | 3 | 141 KB image/png 1050x853 |
| 20 | 7.6 | 7.9 | type_text 1.10 | 254+19968 | 66 | 0 | 0 | 3 |  |
| 21 | 19.0 | 19.5 | get_window_state 0.32 | 236+20096 | 99 | 0 | 0 | 3 |  |
| 22 | 30.3 | 30.8 |  | 1339+20224 | 41 | 0 | 0 | 4 | 138 KB image/png 1050x853 |

model total 155.8 s, tool total 18.3 s over 22 turns; gate: 12 actions exec 12.3 s, gate decision 0.11 s, 10 observations 5.3 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow04-chrome-af630914-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.6 | 2.8 | list_apps 0.03 | 109+4224 | 36 | 0 | 0 | 0 |  |
| 2 | 2.1 | 2.4 | list_windows 0.01 | 1784+4224 | 55 | 10 | 40 | 0 |  |
| 3 | 2.0 | 2.3 | get_window_state 0.25 | 225+5888 | 64 | 0 | 0 | 0 |  |
| 4 | 3.9 | 4.7 | click 1.36 | 2661+6016 | 147 | 63 | 281 | 1 | 57 KB image/png 1050x853 |
| 5 | 3.3 | 3.7 | type_text 1.12 | 3114+8576 | 83 | 0 | 0 | 1 |  |
| 6 | 6.8 | 7.3 | press_key 1.36 | 169+11648 | 79 | 0 | 0 | 1 |  |
| 7 | 12.5 | 13.0 | get_window_state 0.34 | 2699+11776 | 97 | 16 | 66 | 1 |  |
| 8 | 3.5 | 3.9 | click 1.40 | 643+14464 | 84 | 0 | 0 | 1 |  |
| 9 | 9.4 | 10.2 | get_window_state 0.08 | 2715+15104 | 125 | 24 | 110 | 1 |  |
| 10 | 18.2 | 19.0 | scroll 2.69 | 747+17792 | 142 | 16 | 75 | 2 | 134 KB image/png 1050x853 |
| 11 | 32.8 | 33.9 | click 2.13 | 3367+18432 | 197 | 103 | 427 | 3 | 143 KB image/png 1050x853 |
| 12 | 43.2 | 43.6 | click 1.78 | 3397+21760 | 111 | 17 | 55 | 4 | 149 KB image/png 1050x853 |
| 13 | 55.0 | 57.0 | click 1.98 | 3341+25088 | 308 | 214 | 954 | 5 | 143 KB image/png 1050x853 |
| 14 | 69.5 | 71.4 | press_key 1.37 | 3482+28416 | 285 | 206 | 840 | 6 | 150 KB image/png 1050x853 |
| 15 | 84.6 | 85.3 | press_key 1.43 | 3449+31872 | 99 | 20 | 96 | 7 | 148 KB image/png 1050x853 |
| 16 | 56.4 | 57.5 | press_key 0.53 | 2801+35200 | 178 | 99 | 467 | 7 |  |
| 17 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 418.0 s, tool total 17.9 s over 17 turns; gate: 11 actions exec 10.6 s, gate decision 0.17 s, 14 observations 6.4 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow05-libreoffice_calc-abed40dc-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 8.6 | 9.0 | list_windows 0.01 | 4341+0 | 40 | 0 | 0 | 0 |  |
| 2 | 4.8 | 4.9 | get_window_state 0.33 | 347+4224 | 71 | 7 | 27 | 0 |  |
| 3 | 13.5 | 19.2 | click 0.32 | 4151+4480 | 907 | 792 | 3157 | 1 | 259 KB image/png 1396x823 |
| 4 | 28.9 | 30.8 | get_window_state 0.18 | 1066+8576 | 360 | 261 | 943 | 1 |  |
| 5 | 4.3 | 10.7 | get_window_state 0.20 | 694+9600 | 789 | 690 | 2939 | 1 |  |
| 6 | 13.1 | 16.3 | press_key 0.85 | 974+10240 | 467 | 360 | 1542 | 1 |  |
| 7 | 13.1 | 13.3 | click 0.02 | 588+11136 | 48 | 0 | 0 | 1 |  |
| 8 | 12.2 | 12.8 | get_window_state 0.10 | 203+11648 | 112 | 12 | 48 | 1 |  |
| 9 | 10.4 | 10.8 | click 0.81 | 1529+11776 | 59 | 11 | 39 | 1 |  |
| 10 | 10.6 | 10.9 | get_window_state 0.38 | 267+13184 | 82 | 0 | 0 | 1 |  |
| 11 | 19.8 | 20.9 | click 0.02 | 4220+13440 | 157 | 109 | 387 | 1 |  |
| 12 | 19.5 | 20.9 | get_window_state 0.08 | 362+17536 | 220 | 137 | 528 | 1 |  |
| 13 | 32.4 | 33.5 | click 1.52 | 562+17792 | 202 | 108 | 324 | 2 | 58 KB image/png 350x594 |
| 14 | 23.1 | 29.4 | press_key 1.53 | 376+18304 | 969 | 890 | 3730 | 2 |  |
| 15 | 30.1 | 30.9 | press_key 0.89 | 1198+18560 | 150 | 71 | 277 | 2 |  |
| 16 | 38.0 | 38.3 | get_window_state 0.06 | 239+19712 | 83 | 0 | 0 | 2 |  |
| 17 | 31.5 | 34.8 | press_key 0.84 | 430+19840 | 448 | 369 | 1460 | 3 | 58 KB image/png 350x594 |
| 18 | 16.4 | 16.9 | get_window_state 0.07 | 537+20224 | 83 | 0 | 0 | 3 |  |
| 19 | 24.7 | 26.4 | run_actions 9.13 | 344+20736 | 421 | 85 | 334 | 4 | 58 KB image/png 350x594 |
| 20 | 54.8 | 55.5 | get_window_state 0.06 | 3461+20992 | 109 | 26 | 115 | 4 |  |
| 21 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 446.2 s, tool total 17.4 s over 21 turns; gate: 24 actions exec 15.3 s, gate decision 0.10 s, 11 observations 1.5 s (5 with screenshot), reviewer calls 0 (0.0 s)

