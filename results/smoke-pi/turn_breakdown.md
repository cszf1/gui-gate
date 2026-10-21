# Per-turn time breakdown (bench/turn_breakdown.py)

## og02-writer-pdf-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.0 | list_apps 0.05 | 83+4224 | 40 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.3 | list_windows 0.01 | 1646+4224 | 58 | 12 | 50 | 0 |  |
| 3 | 2.4 | 3.0 | get_window_state 0.31 | 227+5760 | 121 | 39 | 168 | 0 |  |
| 4 | 27.7 | 29.0 | click 0.18 | 4675+5888 | 198 | 133 | 543 | 1 | 313 KB image/png 1396x823 |
| 5 | 11.0 | 11.4 | get_window_state 0.28 | 352+10496 | 98 | 34 | 117 | 1 |  |
| 6 | 28.5 | 29.6 | click 0.88 | 1782+10752 | 201 | 117 | 544 | 2 | 67 KB image/png 782x444 |
| 7 | 7.9 | 8.4 | get_window_state 2.06 | 384+12416 | 64 | 0 | 0 | 2 |  |
| 8 | 28.7 | 29.6 | click 5.12 | 484+12800 | 161 | 67 | 256 | 3 | 17 KB image/png 546x115 |
| 9 | 8.0 | 8.6 | get_window_state 0.14 | 330+13184 | 83 | 0 | 0 | 3 |  |
| 10 | 47.7 | 50.1 | get_window_state 1.01 | 1023+13440 | 345 | 246 | 998 | 4 | 272 KB image/png 1396x823 |
| 11 | 5.7 | 9.6 | click 0.75 | 676+14336 | 693 | 599 | 2491 | 4 |  |
| 12 | 43.4 | 43.9 | get_window_state 0.14 | 815+14976 | 83 | 0 | 0 | 4 |  |
| 13 | 34.7 | 36.8 | click 0.02 | 879+15744 | 244 | 179 | 818 | 5 | 314 KB image/png 1396x823 |
| 14 | 85.7 | 86.2 | get_window_state 0.38 | 434+16512 | 118 | 0 | 0 | 5 |  |
| 15 | 57.8 | 60.0 | click 0.03 | 354+16896 | 65 | 0 | 0 | 5 |  |
| 16 | 56.6 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 410.5 s, tool total 11.4 s over 16 turns; gate: 6 actions exec 6.9 s, gate decision 0.03 s, 8 observations 4.0 s (5 with screenshot), reviewer calls 0 (0.0 s)

## og02-writer-pdf-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.2 | list_windows 0.01 | 83+4224 | 39 | 0 | 0 | 0 |  |
| 2 | 2.9 | 3.7 | get_window_state 0.16, get_window_state 0.17 | 343+4224 | 194 | 40 | 154 | 0 |  |
| 3 | 1.8 | 3.3 | click 0.46 | 4146+4480 | 112 | 64 | 271 | 0 |  |
| 4 | 2.5 | 3.0 | click 1.79 | 204+8576 | 73 | 25 | 93 | 0 |  |
| 5 | 1.9 | 2.4 | click 0.01 | 1558+8704 | 95 | 47 | 199 | 0 |  |
| 6 | 1.7 | 6.1 | click 2.02 | 243+10240 | 691 | 643 | 2831 | 0 |  |
| 7 | 2.8 | 3.3 | press_key 1.19 | 2195+10368 | 116 | 15 | 55 | 0 |  |
| 8 | 2.5 | 2.9 | type_text 0.03 | 1501+12544 | 91 | 0 | 0 | 0 |  |
| 9 | 3.2 | 3.6 | click 1.70 | 238+13952 | 87 | 23 | 92 | 0 |  |
| 10 | 2.3 | 5.5 | run_actions 3.86 | 1606+14080 | 696 | 222 | 961 | 0 |  |
| 11 | 2.7 | 3.0 | type_text 0.03 | 2321+15616 | 125 | 34 | 157 | 0 |  |
| 12 | 2.0 | 2.6 | click 0.97 | 188+17920 | 48 | 0 | 0 | 0 |  |
| 13 | 2.6 | 3.6 | click 0.34 | 180+18048 | 145 | 97 | 425 | 0 |  |
| 14 | 4.8 | 6.2 | get_window_state 0.20 | 2863+18176 | 262 | 179 | 844 | 0 |  |
| 15 | 27.1 | 27.2 | click 1.59 | 1058+20992 | 121 | 27 | 108 | 1 | 314 KB image/png 1396x823 |
| 16 | 26.8 | 28.2 | click 1.14 | 1962+22016 | 174 | 125 | 532 | 2 | 76 KB image/png 782x444 |
| 17 | 25.7 | 26.4 |  | 289+23936 | 100 | 0 | 0 | 2 |  |

model total 133.2 s, tool total 15.6 s over 17 turns; gate: 16 actions exec 10.2 s, gate decision 1.31 s, 11 observations 3.0 s (2 with screenshot), reviewer calls 1 (0.7 s)

## ow01-libreoffice_calc-7efeb4b1-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.3 | list_apps 0.35, list_windows 0.36 | 3945+384 | 58 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.4 | get_window_state 0.44 | 1942+4224 | 110 | 10 | 42 | 0 |  |
| 3 | 20.8 | 30.2 | click 0.32 | 4112+6144 | 1508 | 1414 | 5182 | 1 | 381 KB image/png 1396x823 |
| 4 | 26.3 | 27.5 | get_window_state 0.16 | 1628+10240 | 194 | 111 | 417 | 1 |  |
| 5 | 30.0 | 31.3 | press_key 0.86 | 1035+11776 | 287 | 189 | 756 | 2 | 383 KB image/png 1396x823 |
| 6 | 28.4 | 28.9 | get_window_state 0.15 | 341+12800 | 83 | 0 | 0 | 2 |  |
| 7 | 39.5 | 45.6 | click 1.53 | 917+13056 | 924 | 830 | 2970 | 3 | 377 KB image/png 1396x823 |
| 8 | 48.1 | 48.6 | get_window_state 0.15 | 1014+13952 | 83 | 0 | 0 | 3 |  |
| 9 | 67.2 | 71.7 | click 1.45 | 950+14848 | 720 | 626 | 2222 | 4 | 381 KB image/png 1396x823 |
| 10 | 87.3 | 87.8 | get_window_state 0.16 | 843+15744 | 83 | 0 | 0 | 4 |  |
| 11 | 82.8 | 83.9 | click 1.42 | 907+16512 | 164 | 70 | 218 | 5 | 382 KB image/png 1396x823 |
| 12 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 460.2 s, tool total 7.3 s over 12 turns; gate: 5 actions exec 5.6 s, gate decision 0.02 s, 6 observations 0.6 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.1 | 2.4 | list_windows 0.01 | 105+4224 | 41 | 0 | 0 | 0 |  |
| 2 | 4.6 | 6.0 | get_window_state 0.26 | 367+4224 | 125 | 49 | 169 | 0 |  |
| 3 | 15.0 | 22.1 | press_key 0.43 | 4216+4480 | 1103 | 981 | 3726 | 1 | 381 KB image/png 1396x823 |
| 4 | 2.8 | 3.3 | get_window_state 0.16 | 3992+8576 | 106 | 23 | 105 | 1 |  |
| 5 | 23.3 | 29.5 | press_key 1.19 | 879+12544 | 982 | 866 | 2728 | 2 | 377 KB image/png 1396x823 |
| 6 | 27.4 | 29.0 | click 0.90 | 3862+13312 | 234 | 140 | 532 | 2 |  |
| 7 | 45.4 | 52.8 | run_actions 3.94 | 7170+17152 | 1207 | 1034 | 3661 | 4 | 381 KB image/png 1396x823; 381 KB image/png 1396x823 |
| 8 | 86.3 | 92.2 | run_actions 5.21 | 4834+24320 | 1065 | 621 | 2190 | 5 | 382 KB image/png 1396x823 |
| 9 | 83.3 | 83.9 | get_window_state 0.25 | 4729+29056 | 95 | 12 | 69 | 5 |  |
| 10 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 321.1 s, tool total 12.4 s over 10 turns; gate: 27 actions exec 8.8 s, gate decision 0.09 s, 10 observations 2.6 s (6 with screenshot), reviewer calls 0 (0.0 s)

