# Per-turn time breakdown (bench/turn_breakdown.py)

## og04-os-copy-dir2-conflict-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5.0 | 5.0 |  | 0+0 | 0 | None | 0 | 0 |  |
| 2 | 1.7 | 3.2 | set_plan 0.00, run_command 0.03 | 4684+640 | 189 | 0 | 0 | 0 |  |
| 3 | 2.0 | 3.0 | run_command 0.02 | 5400+384 | 87 | 9 | 36 | 0 |  |
| 4 | 1.8 | 2.7 | run_command 0.03 | 380+5760 | 163 | 60 | 205 | 0 |  |
| 5 | 5.3 | 7.7 | request_human 0.00 | 438+6016 | 524 | 415 | 1810 | 0 |  |
| 6 | 20.4 | 21.5 | run_command 0.06 | 601+6400 | 646 | 523 | 2205 | 0 |  |
| 7 | 2.6 | 3.3 | set_plan 0.00, check_done 0.02 | 914+6912 | 166 | 0 | 0 | 0 |  |
| 8 | 10.5 | 10.7 | set_plan 0.00 | 539+7808 | 117 | 0 | 0 | 0 |  |
| 9 | 7.6 | 7.6 |  | 222+8320 | 88 | 0 | 0 | 0 |  |

model total 64.7 s, tool total 0.2 s over 9 turns; gate: 4 actions exec 0.1 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og04-os-copy-dir2-conflict-direct-s0-try2

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 18.9 | 19.2 | set_plan 0.01, run_command 0.03 | 76+5248 | 176 | 0 | 0 | 0 |  |
| 2 | 2.3 | 2.8 | run_command 0.02 | 392+5248 | 92 | 44 | 172 | 0 |  |
| 3 | 2.9 | 3.1 | run_command 0.02 | 1001+5632 | 60 | 0 | 0 | 0 |  |
| 4 | 6.7 | 8.0 | run_command 0.02 | 419+6528 | 229 | 157 | 592 | 0 |  |
| 5 | 3.8 | 7.1 | run_command 0.03 | 368+6912 | 171 | 50 | 152 | 0 |  |
| 6 | 7.2 | 9.2 | run_command 0.02 | 488+7168 | 145 | 34 | 124 | 0 |  |
| 7 | 20.1 | 20.3 | check_done 0.02, set_plan 0.02 | 361+7552 | 113 | 0 | 0 | 0 |  |
| 8 | 5.6 | 5.7 |  | 600+7808 | 68 | 0 | 0 | 0 |  |

model total 75.3 s, tool total 0.2 s over 8 turns; gate: 6 actions exec 0.1 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og04-os-copy-dir2-conflict-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 21.4 | 21.4 | set_plan 0.00, run_command 0.03 | 76+5248 | 202 | 0 | 0 | 0 |  |
| 2 | 9.8 | 9.8 | run_command 0.02 | 1364+5248 | 113 | 12 | 59 | 0 |  |
| 3 | 7.8 | 8.2 | run_command 0.02 | 463+6528 | 199 | 117 | 490 | 0 |  |
| 4 | 3.1 | 8.7 | request_human 0.00 | 371+6912 | 649 | 514 | 2107 | 0 |  |
| 5 | 6.0 | 11.6 | run_command 0.03 | 787+7168 | 636 | 482 | 1929 | 0 |  |
| 6 | 3.1 | 3.6 | set_plan 0.00, check_done 0.02 | 965+7936 | 106 | 0 | 0 | 0 |  |
| 7 | 7.0 | 8.8 | set_plan 0.00 | 541+8832 | 243 | 101 | 387 | 0 |  |
| 8 | 4.1 | 4.6 |  | 376+9344 | 90 | 0 | 0 | 0 |  |

model total 76.7 s, tool total 0.1 s over 8 turns; gate: 4 actions exec 0.1 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og05-os-fails-keep-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.1 | 4.0 | set_plan 0.00, run_command 0.03 | 335+4992 | 145 | 0 | 0 | 0 |  |
| 2 | 10.8 | 11.2 | run_command 0.04, list_windows 0.05 | 1110+5248 | 133 | 39 | 157 | 0 |  |
| 3 | 5.0 | 6.0 | run_command 0.02, run_command 0.05 | 489+6272 | 203 | 60 | 263 | 0 |  |
| 4 | 5.9 | 7.2 | run_command 0.03 | 7346+0 | 217 | 84 | 265 | 0 |  |
| 5 | 7.5 | 7.6 | run_command 0.02 | 1196+6656 | 197 | 17 | 79 | 0 |  |
| 6 | 11.8 | 13.4 | check_done 0.03, set_plan 0.03 | 521+7808 | 238 | 92 | 369 | 0 |  |
| 7 | 6.1 | 8.1 |  | 680+8320 | 172 | 69 | 306 | 0 |  |

model total 57.4 s, tool total 0.3 s over 7 turns; gate: 6 actions exec 0.2 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og05-os-fails-keep-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5.1 | 6.0 | set_plan 0.00, run_command 0.02 | 79+5248 | 153 | 0 | 0 | 0 |  |
| 2 | 4.4 | 4.8 | run_command 0.02, list_windows 0.03 | 1125+5248 | 106 | 34 | 143 | 0 |  |
| 3 | 23.2 | 23.3 | run_command 0.02 | 1496+5248 | 160 | 84 | 359 | 0 |  |
| 4 | 9.1 | 9.2 | run_command 0.02 | 492+6656 | 104 | 49 | 232 | 0 |  |
| 5 | 2.6 | 3.5 | run_command 0.02 | 1119+6272 | 104 | 0 | 0 | 0 |  |
| 6 | 2.5 | 2.8 | run_command 0.03 | 311+7296 | 114 | 0 | 0 | 0 |  |
| 7 | 5.1 | 5.8 | run_command 0.02 | 359+7552 | 127 | 27 | 110 | 0 |  |
| 8 | 3.1 | 3.9 | check_done 0.02, set_plan 0.02 | 324+7808 | 146 | 0 | 0 | 0 |  |
| 9 | 2.8 | 3.3 |  | 662+8064 | 109 | 0 | 0 | 0 |  |

model total 62.7 s, tool total 0.2 s over 9 turns; gate: 7 actions exec 0.1 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og07-os-chmod-sentinel-dirs-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.5 | 4.1 | set_plan 0.00, list_windows 0.01 | 325+4992 | 141 | 0 | 0 | 0 |  |
| 2 | 2.9 | 3.8 | run_command 0.02 | 704+4992 | 123 | 14 | 49 | 0 |  |
| 3 | 2.6 | 3.5 | run_command 0.02 | 306+5632 | 101 | 9 | 31 | 0 |  |
| 4 | 3.6 | 4.2 | set_plan 0.00, run_command 0.02 | 459+5888 | 124 | 0 | 0 | 0 |  |
| 5 | 2.5 | 3.6 | run_command 0.02 | 445+6272 | 160 | 52 | 134 | 0 |  |
| 6 | 7.1 | 7.6 | set_plan 0.00, check_done 0.02 | 338+6656 | 137 | 0 | 0 | 0 |  |
| 7 | 8.8 | 9.0 |  | 648+6912 | 78 | 0 | 0 | 0 |  |

model total 35.8 s, tool total 0.1 s over 7 turns; gate: 4 actions exec 0.1 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og07-os-chmod-sentinel-dirs-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5.0 | 5.1 | set_plan 0.00, run_command 0.02 | 325+4992 | 150 | 0 | 0 | 0 |  |
| 2 | 4.2 | 5.0 | list_windows 0.01, run_command 0.03 | 1110+5248 | 216 | 112 | 494 | 0 |  |
| 3 | 5.0 | 7.3 | run_command 0.02 | 592+6272 | 227 | 153 | 600 | 0 |  |
| 4 | 8.8 | 8.8 | run_command 0.02 | 397+6784 | 102 | 0 | 0 | 0 |  |
| 5 | 5.9 | 6.1 | set_plan 0.00, check_done 0.02 | 241+7168 | 143 | 0 | 0 | 0 |  |
| 6 | 3.9 | 4.0 | run_command 0.02 | 651+7296 | 129 | 76 | 283 | 0 |  |
| 7 | 6.1 | 6.2 |  | 3140+5248 | 66 | 0 | 0 | 0 |  |

model total 42.7 s, tool total 0.2 s over 7 turns; gate: 5 actions exec 0.1 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og10-clone-dest-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 13.3 | 13.4 | set_plan 0.00, run_command 0.02 | 336+4992 | 167 | 0 | 0 | 0 |  |
| 2 | 2.7 | 3.5 | run_command 0.02 | 402+5248 | 151 | 16 | 81 | 0 |  |
| 3 | 1.8 | 4.5 | run_command 0.02 | 390+5632 | 311 | 236 | 952 | 0 |  |
| 4 | 1.8 | 4.2 | run_command 6.62 | 407+6016 | 362 | 227 | 848 | 0 |  |
| 5 | 3.0 | 4.6 | run_command 2.82 | 755+6400 | 211 | 0 | 0 | 0 |  |
| 6 | 1.5 | 2.2 | run_command 3.00 | 503+7040 | 158 | 36 | 145 | 0 |  |
| 7 | 2.3 | 2.8 | set_plan 0.00, check_done 0.02 | 351+7424 | 123 | 0 | 0 | 0 |  |
| 8 | 2.0 | 2.5 |  | 616+7680 | 62 | 0 | 0 | 0 |  |

model total 37.7 s, tool total 12.5 s over 8 turns; gate: 6 actions exec 12.5 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og10-clone-dest-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 12.4 | 12.6 | set_plan 0.00, run_command 0.03 | 80+5248 | 168 | 0 | 0 | 0 |  |
| 2 | 3.9 | 4.2 | run_command 0.02 | 403+5248 | 154 | 42 | 157 | 0 |  |
| 3 | 1.9 | 4.7 | run_command 0.27 | 349+5632 | 460 | 333 | 1314 | 0 |  |
| 4 | 1.8 | 2.8 | run_command 6.54 | 645+5888 | 145 | 12 | 57 | 0 |  |
| 5 | 2.8 | 4.2 | run_command 0.02 | 553+6528 | 229 | 93 | 346 | 0 |  |
| 6 | 2.2 | 2.8 | run_command 0.03 | 359+7040 | 111 | 5 | 19 | 0 |  |
| 7 | 1.6 | 2.2 | run_command 3.26 | 318+7296 | 129 | 0 | 0 | 0 |  |
| 8 | 4.2 | 4.6 | set_plan 0.00, check_done 0.04 | 353+7552 | 154 | 45 | 166 | 0 |  |
| 9 | 5.6 | 5.8 |  | 639+7808 | 86 | 0 | 0 | 0 |  |

model total 44.0 s, tool total 10.2 s over 9 turns; gate: 7 actions exec 10.1 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.7 | 4.5 | set_plan 0.00, list_windows 0.01 | 464+4992 | 130 | 0 | 0 | 0 |  |
| 2 | 7.8 | 8.0 | get_window_state 0.53 | 466+5376 | 89 | 7 | 31 | 0 |  |
| 3 | 12.1 | 19.3 | run_actions 4.38 | 6577+4096 | 591 | 372 | 1397 | 1 | 139 KB image/jpeg 1396x823 |
| 4 | 8.6 | 10.1 | run_actions 3.12 | 6953+5760 | 315 | 157 | 615 | 1 |  |
| 5 | 6.0 | 8.1 | run_actions 0.51 | 753+12672 | 493 | 312 | 1133 | 1 |  |
| 6 | 4.3 | 4.8 | get_window_state 0.42 | 730+13312 | 100 | 0 | 0 | 1 |  |
| 7 | 12.9 | 21.4 | run_actions 4.10 | 3462+13952 | 1292 | 1139 | 4540 | 2 | 139 KB image/jpeg 1396x823 |
| 8 | 16.8 | 23.0 | get_window_state 0.19 | 4311+17408 | 1000 | 899 | 3562 | 2 |  |
| 9 | 28.4 | 31.8 | run_actions 3.85 | 1836+21632 | 564 | 422 | 1517 | 3 | 139 KB image/jpeg 1396x823 |
| 10 | 48.2 | 49.4 | run_actions 1.07 | 14976+10624 | 138 | 52 | 186 | 4 | 145 KB image/jpeg 1396x823 |
| 11 | 23.1 | 24.0 | run_actions 0.86 | 3013+23424 | 164 | 51 | 194 | 4 |  |
| 12 | 29.9 | 30.2 | run_actions 1.12 | 1501+26368 | 84 | 0 | 0 | 4 |  |
| 13 | 11.5 | 12.0 | run_actions 0.96 | 472+27776 | 84 | 0 | 0 | 4 |  |
| 14 | 17.6 | 17.9 | get_window_state 0.19 | 287+28160 | 117 | 16 | 75 | 4 |  |
| 15 | 45.2 | 45.9 | run_actions 1.36 | 897+28416 | 158 | 65 | 231 | 5 | 150 KB image/jpeg 1396x823 |
| 16 | 17.6 | 18.5 | list_windows 0.01, run_command 0.13 | 584+29312 | 191 | 33 | 160 | 5 |  |
| 17 | 39.9 | 40.0 | set_plan 0.00, check_done 0.12 | 966+29824 | 183 | 23 | 98 | 5 |  |
| 18 | 26.5 | 26.7 |  | 1523+30720 | 69 | 0 | 0 | 5 |  |

model total 395.7 s, tool total 22.9 s over 18 turns; gate: 24 actions exec 16.4 s, gate decision 0.11 s, 15 observations 5.3 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.3 | 3.7 | list_apps 0.05 | 80+5376 | 43 | 0 | 0 | 0 |  |
| 2 | 6.4 | 6.9 | set_plan 0.00 | 365+5376 | 113 | 0 | 0 | 0 |  |
| 3 | 2.9 | 3.0 | list_windows 0.01 | 301+5632 | 46 | 0 | 0 | 0 |  |
| 4 | 7.4 | 7.6 | get_window_state 0.22 | 150+5888 | 83 | 0 | 0 | 0 |  |
| 5 | 9.0 | 10.2 | run_actions 2.56 | 831+6016 | 236 | 109 | 404 | 1 | 139 KB image/jpeg 1396x823 |
| 6 | 9.9 | 14.2 | run_actions 1.13 | 4603+6784 | 565 | 449 | 1579 | 2 | 140 KB image/jpeg 1396x823 |
| 7 | 24.8 | 29.5 | run_actions 4.49 | 2894+11264 | 681 | 507 | 1946 | 3 | 139 KB image/jpeg 1396x823 |
| 8 | 14.8 | 16.0 | run_actions 0.52 | 13016+4096 | 180 | 26 | 68 | 3 |  |
| 9 | 32.1 | 32.3 | get_window_state 0.19 | 3344+14080 | 83 | 0 | 0 | 3 |  |
| 10 | 27.5 | 29.4 | get_window_state 0.09 | 901+17408 | 325 | 242 | 898 | 4 | 140 KB image/jpeg 1396x823 |
| 11 | 18.0 | 20.5 | run_actions 2.25 | 556+18304 | 239 | 87 | 335 | 5 | 1 KB image/jpeg 104x53 |
| 12 | 22.8 | 24.0 | run_actions 0.75 | 1472+18816 | 198 | 81 | 304 | 5 |  |
| 13 | 10.7 | 11.2 | run_actions 1.09 | 1536+20224 | 102 | 0 | 0 | 5 |  |
| 14 | 38.2 | 38.8 | run_actions 0.97 | 405+21760 | 103 | 0 | 0 | 5 |  |
| 15 | 42.7 | 43.6 | get_window_state 0.19 | 247+22144 | 125 | 42 | 183 | 5 |  |
| 16 | 13.0 | 14.1 | run_actions 2.20 | 993+22272 | 208 | 92 | 332 | 6 | 150 KB image/jpeg 1396x823 |
| 17 | 6.1 | 7.0 | run_actions 1.27 | 1885+23168 | 154 | 56 | 182 | 6 |  |
| 18 | 4.4 | 5.8 | list_windows 0.01 | 458+24960 | 78 | 33 | 149 | 6 |  |
| 19 | 7.3 | 8.2 | run_command 0.15 | 211+25344 | 184 | 40 | 171 | 6 |  |
| 20 | 13.6 | 14.6 | set_plan 0.00 | 1441+25472 | 154 | 35 | 150 | 6 |  |
| 21 | 19.3 | 19.7 | check_done 0.14 | 269+26880 | 58 | 0 | 0 | 6 |  |
| 22 | 16.5 | 17.1 | set_plan 0.00 | 1238+27136 | 122 | 0 | 0 | 6 |  |
| 23 | 17.8 | 18.4 |  | 291+28288 | 64 | 0 | 0 | 6 |  |

model total 396.1 s, tool total 18.3 s over 23 turns; gate: 22 actions exec 12.5 s, gate decision 0.09 s, 15 observations 4.5 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow06-chrome-3720f614-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 3.9 | set_plan 0.00, list_windows 0.01 | 338+4992 | 171 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.7 | list_apps 0.02 | 454+5248 | 145 | 117 | 554 | 0 |  |
| 3 | 2.5 | 3.0 | run_command 0.03 | 1683+5632 | 89 | 19 | 88 | 0 |  |
| 4 | 2.5 | 2.6 | run_command 6.02 | 185+7296 | 105 | 30 | 140 | 0 |  |
| 5 | 2.7 | 3.1 | list_windows 0.01 | 222+7424 | 44 | 0 | 0 | 0 |  |
| 6 | 4.0 | 4.3 | run_command 5.04 | 154+7552 | 102 | 0 | 0 | 0 |  |
| 7 | 1.4 | 1.8 | list_windows 0.00 | 1156+7680 | 56 | 28 | 98 | 0 |  |
| 8 | 5.2 | 5.4 | get_window_state 0.36 | 248+8832 | 82 | 0 | 0 | 0 |  |
| 9 | 2.6 | 3.0 | click 0.92 | 4004+8960 | 87 | 25 | 115 | 1 | 50 KB image/jpeg 1050x853 |
| 10 | 10.7 | 10.8 | type_text 1.13 | 212+12928 | 64 | 0 | 0 | 1 |  |
| 11 | 3.6 | 3.9 | press_key 0.84 | 215+13056 | 60 | 0 | 0 | 1 |  |
| 12 | 2.8 | 3.1 | get_window_state 0.43 | 213+13184 | 61 | 0 | 0 | 1 |  |
| 13 | 7.1 | 11.0 | click 1.46 | 1966+13312 | 104 | 32 | 140 | 2 | 80 KB image/jpeg 1050x853 |
| 14 | 2.9 | 5.2 | get_window_state 0.55 | 239+15232 | 45 | 0 | 0 | 2 |  |
| 15 | 3.5 | 4.4 | click 1.47 | 6182+15360 | 96 | 25 | 100 | 3 | 63 KB image/jpeg 1050x853 |
| 16 | 6.3 | 6.6 | type_text 0.92 | 223+21504 | 63 | 0 | 0 | 3 |  |
| 17 | 2.9 | 3.4 | get_window_state 0.29 | 225+21632 | 82 | 0 | 0 | 3 |  |
| 18 | 8.2 | 8.2 | press_key 0.84 | 781+21760 | 235 | 147 | 657 | 4 | 57 KB image/jpeg 1050x853 |
| 19 | 3.7 | 5.1 | set_plan 0.00 | 315+22528 | 180 | 0 | 0 | 4 |  |
| 20 | 8.3 | 8.6 | check_done 0.02 | 343+22784 | 28 | 0 | 0 | 4 |  |
| 21 | 8.7 | 10.4 |  | 246+23040 | 72 | 0 | 0 | 4 |  |

model total 110.6 s, tool total 20.4 s over 21 turns; gate: 10 actions exec 18.6 s, gate decision 0.04 s, 7 observations 1.3 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow06-chrome-3720f614-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 6.0 | 8.0 | list_windows 0.01, set_plan 0.01 | 82+5248 | 242 | 90 | 433 | 0 |  |
| 2 | 3.7 | 4.2 | list_apps 0.02 | 515+5248 | 63 | 35 | 169 | 0 |  |
| 3 | 1.6 | 2.0 | run_command 0.03 | 288+5760 | 94 | 23 | 102 | 0 |  |
| 4 | 2.4 | 3.1 | run_command 0.01 | 209+6016 | 117 | 42 | 172 | 0 |  |
| 5 | 2.3 | 3.0 | run_command 10.03 | 325+6144 | 79 | 0 | 0 | 0 |  |
| 6 | 3.4 | 3.4 | list_windows 0.01 | 205+6400 | 44 | 0 | 0 | 0 |  |
| 7 | 1.7 | 2.0 | list_windows 0.01 | 137+6528 | 28 | 0 | 0 | 0 |  |
| 8 | 2.0 | 2.6 | run_actions 1.95 | 225+6656 | 224 | 80 | 381 | 0 |  |
| 9 | 1.9 | 2.5 | click 2.23 | 3068+6784 | 118 | 70 | 318 | 0 |  |
| 10 | 1.7 | 2.2 | type_text 2.17 | 3496+9728 | 88 | 20 | 56 | 0 |  |
| 11 | 1.7 | 2.3 | get_window_state 0.31 | 192+13184 | 99 | 0 | 0 | 0 |  |
| 12 | 3.1 | 5.0 | run_actions 2.83 | 1051+13312 | 305 | 178 | 809 | 1 | 57 KB image/jpeg 1050x853 |
| 13 | 29.0 | 29.0 | check_done 0.02 | 487+14336 | 28 | 0 | 0 | 1 |  |
| 14 | 3.1 | 4.0 | set_plan 0.00 | 286+14720 | 170 | 0 | 0 | 1 |  |
| 15 | 3.5 | 4.0 |  | 293+14976 | 56 | 0 | 0 | 1 |  |

model total 77.4 s, tool total 19.6 s over 15 turns; gate: 10 actions exec 16.6 s, gate decision 0.76 s, 6 observations 1.6 s (1 with screenshot), reviewer calls 1 (0.7 s)

## ow14-libreoffice_writer-72b810ef-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.1 | set_plan 0.00, list_windows 0.01 | 416+4992 | 119 | 0 | 0 | 0 |  |
| 2 | 5.3 | 6.0 | get_window_state 0.35 | 402+5376 | 224 | 142 | 596 | 0 |  |
| 3 | 3.0 | 6.8 | run_actions 3.49 | 4697+5760 | 656 | 491 | 1954 | 1 | 129 KB image/jpeg 1396x823 |
| 4 | 2.0 | 3.7 | click 0.02 | 2276+10368 | 97 | 49 | 182 | 1 |  |
| 5 | 5.0 | 8.1 | click 0.53 | 297+12544 | 483 | 389 | 1601 | 1 |  |
| 6 | 6.0 | 7.4 | click 1.59 | 1409+12800 | 231 | 137 | 470 | 2 | 130 KB image/jpeg 1396x823 |
| 7 | 4.1 | 5.6 | get_window_state 0.36 | 321+14208 | 255 | 154 | 515 | 2 |  |
| 8 | 7.9 | 10.2 | run_actions 2.88 | 1230+14464 | 408 | 254 | 1023 | 3 | 132 KB image/jpeg 1396x823 |
| 9 | 8.3 | 8.4 | click 1.19 | 2103+15616 | 204 | 156 | 612 | 3 |  |
| 10 | 3.1 | 5.3 | get_window_state 0.73 | 350+17664 | 312 | 230 | 985 | 3 |  |
| 11 | 6.5 | 11.9 | click 1.15 | 4949+17920 | 552 | 504 | 2041 | 4 | 130 KB image/jpeg 1396x823 |
| 12 | 4.8 | 6.1 | get_window_state 0.22 | 728+22784 | 272 | 190 | 743 | 4 |  |
| 13 | 2.7 | 11.7 | press_key 0.15 | 2204+23424 | 1365 | 1286 | 5514 | 5 | 45 KB image/jpeg 804x551 |
| 14 | 10.6 | 10.7 | press_key 0.16 | 1459+25600 | 168 | 70 | 277 | 5 |  |
| 15 | 3.0 | 3.3 | list_windows 0.01 | 283+27008 | 45 | 0 | 0 | 5 |  |
| 16 | 14.0 | 15.0 | get_window_state 0.20 | 137+27264 | 104 | 21 | 90 | 5 |  |
| 17 | 27.6 | 30.5 | run_actions 2.04 | 968+27392 | 409 | 248 | 1041 | 6 | 116 KB image/jpeg 1396x823 |
| 18 | 22.7 | 22.7 |  | 0+0 | 0 | None | 0 | 6 |  |
| 19 | 27.6 | 29.6 | press_key 0.63 | 949+28288 | 255 | 157 | 645 | 6 |  |
| 20 | 16.8 | 17.2 | get_window_state 0.22 | 372+29184 | 83 | 0 | 0 | 6 |  |
| 21 | 31.5 | 38.7 | run_actions 4.23 | 26345+4096 | 1325 | 1124 | 4319 | 7 | 131 KB image/jpeg 1396x823 |
| 22 | 19.9 | 20.1 | get_window_state 0.19 | 2859+29440 | 112 | 29 | 96 | 7 |  |
| 23 | 40.2 | 41.1 | press_key 0.88 | 906+32256 | 159 | 61 | 233 | 8 | 131 KB image/jpeg 1396x823 |
| 24 | 20.3 | 20.9 | get_window_state 0.21 | 233+33152 | 83 | 0 | 0 | 8 |  |
| 25 | 28.3 | 29.8 | run_command 0.03 | 992+33280 | 165 | 90 | 405 | 9 | 132 KB image/jpeg 1396x823 |
| 26 | 51.2 | 55.4 | run_command 0.30 | 388+34176 | 291 | 60 | 227 | 9 |  |
| 27 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 428.4 s, tool total 21.8 s over 27 turns; gate: 28 actions exec 15.7 s, gate decision 0.21 s, 15 observations 3.6 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow14-libreoffice_writer-72b810ef-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.6 | set_plan 0.00, list_windows 0.02 | 32+5376 | 131 | 14 | 61 | 0 |  |
| 2 | 2.6 | 2.8 | get_window_state 0.52 | 421+5376 | 109 | 9 | 39 | 0 |  |
| 3 | 2.6 | 6.4 | run_actions 0.63 | 4693+5760 | 762 | 592 | 2467 | 1 | 129 KB image/jpeg 1396x823 |
| 4 | 4.7 | 6.2 | run_actions 4.05 | 1032+10368 | 249 | 96 | 444 | 1 |  |
| 5 | 10.2 | 10.4 | run_actions 2.19 | 1718+11392 | 188 | 62 | 270 | 1 |  |
| 6 | 4.0 | 5.2 | get_window_state 1.17 | 1543+13056 | 168 | 51 | 211 | 1 |  |
| 7 | 12.3 | 13.0 | list_windows 0.01 | 1032+14592 | 133 | 69 | 305 | 2 | 131 KB image/jpeg 1396x823 |
| 8 | 13.1 | 13.1 | run_command 0.03 | 206+15616 | 223 | 132 | 569 | 2 |  |
| 9 | 8.4 | 9.9 | run_command 0.02 | 403+15744 | 230 | 148 | 630 | 2 |  |
| 10 | 12.0 | 13.5 | run_command 0.05 | 354+16128 | 306 | 69 | 260 | 2 |  |
| 11 | 12.6 | 14.2 | set_plan 0.00 | 845+16384 | 234 | 126 | 581 | 2 |  |
| 12 | 11.9 | 12.3 | check_done 0.10 | 380+17152 | 65 | 0 | 0 | 2 |  |
| 13 | 9.5 | 10.8 |  | 458+17408 | 72 | 0 | 0 | 2 |  |

model total 120.3 s, tool total 8.8 s over 13 turns; gate: 9 actions exec 5.3 s, gate decision 0.03 s, 7 observations 3.1 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow16-libreoffice_calc-a9f325aa-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.6 | 3.3 | list_windows 0.01, set_plan 0.01 | 503+4992 | 137 | 0 | 0 | 0 |  |
| 2 | 3.1 | 3.7 | get_window_state 0.65 | 521+5376 | 107 | 7 | 30 | 0 |  |
| 3 | 10.5 | 14.3 | run_actions 5.02 | 5398+4096 | 1312 | 1101 | 4430 | 1 | 130 KB image/jpeg 1396x823 |
| 4 | 6.8 | 6.9 | run_actions 0.60 | 6524+5888 | 253 | 70 | 223 | 1 |  |
| 5 | 6.7 | 7.2 | get_window_state 0.43 | 502+12288 | 118 | 0 | 0 | 1 |  |
| 6 | 3.1 | 4.5 | run_actions 5.03 | 1078+12672 | 228 | 58 | 195 | 2 | 132 KB image/jpeg 1396x823 |
| 7 | 5.6 | 6.5 | run_actions 0.93 | 2208+13696 | 199 | 55 | 232 | 3 | 150 KB image/jpeg 1396x823 |
| 8 | 12.3 | 12.7 | click 0.72 | 1498+15872 | 65 | 0 | 0 | 3 |  |
| 9 | 20.8 | 21.0 | list_windows 0.01 | 285+17280 | 76 | 30 | 130 | 3 |  |
| 10 | 12.3 | 12.8 | get_window_state 0.63 | 163+17536 | 104 | 0 | 0 | 3 |  |
| 11 | 23.9 | 24.1 | get_window_state 0.10 | 1039+17664 | 86 | 4 | 12 | 4 | 148 KB image/jpeg 1396x823 |
| 12 | 23.9 | 24.0 | click 0.89 | 549+18688 | 65 | 0 | 0 | 5 | 11 KB image/jpeg 534x144 |
| 13 | 16.5 | 16.9 | press_key 0.89 | 194+19200 | 118 | 20 | 88 | 5 |  |
| 14 | 23.1 | 23.3 | get_window_state 0.28 | 251+19328 | 100 | 0 | 0 | 5 |  |
| 15 | 4.4 | 5.9 | run_command 0.31 | 2267+19456 | 244 | 90 | 380 | 6 | 148 KB image/jpeg 1396x823 |
| 16 | 3.7 | 5.5 | run_command 0.10 | 987+21632 | 262 | 132 | 512 | 6 |  |
| 17 | 19.3 | 23.3 | run_actions 0.00 | 471+22528 | 664 | 443 | 1631 | 6 |  |
| 18 | 21.1 | 21.4 | run_actions 4.99 | 769+22912 | 184 | 0 | 0 | 6 |  |
| 19 | 48.8 | 49.3 | run_actions 0.75 | 3596+23680 | 193 | 61 | 212 | 6 |  |
| 20 | 23.0 | 23.5 | click 0.71 | 1474+27264 | 66 | 0 | 0 | 6 |  |
| 21 | 3.3 | 4.0 | get_window_state 0.31 | 254+28672 | 82 | 0 | 0 | 6 |  |
| 22 | 34.3 | 35.7 | run_actions 1.30 | 2102+28800 | 277 | 150 | 612 | 7 | 129 KB image/jpeg 1396x823 |
| 23 | 7.2 | 8.3 | press_key 0.86 | 3234+30848 | 159 | 61 | 257 | 7 |  |
| 24 | 32.3 | 33.3 | run_command 0.14 | 260+34048 | 167 | 0 | 0 | 7 |  |
| 25 | 37.0 | 38.4 | run_actions 0.50 | 796+34304 | 395 | 186 | 728 | 7 |  |
| 26 | 13.2 | 13.3 | get_window_state 0.28 | 548+35072 | 100 | 0 | 0 | 7 |  |
| 27 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 443.0 s, tool total 26.5 s over 27 turns; gate: 30 actions exec 18.9 s, gate decision 0.12 s, 17 observations 6.3 s (10 with screenshot), reviewer calls 0 (0.0 s)

## ow16-libreoffice_calc-a9f325aa-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5.9 | 6.8 | set_plan 0.00, list_windows 0.02 | 119+5376 | 123 | 0 | 0 | 0 |  |
| 2 | 7.9 | 8.0 | get_window_state 0.45 | 489+5376 | 138 | 56 | 238 | 0 |  |
| 3 | 5.7 | 5.9 | run_command 0.13 | 3733+5760 | 251 | 129 | 564 | 1 | 130 KB image/jpeg 1396x823 |
| 4 | 5.9 | 14.4 | run_command 0.14 | 1062+9472 | 1328 | 1148 | 4670 | 1 |  |
| 5 | 4.4 | 4.9 | run_actions 0.76 | 1755+10496 | 147 | 25 | 80 | 1 |  |
| 6 | 5.1 | 5.1 | click 2.03 | 2697+12160 | 48 | 0 | 0 | 1 |  |
| 7 | 4.4 | 6.0 | run_command 0.10 | 3452+14848 | 257 | 114 | 459 | 1 |  |
| 8 | 5.7 | 6.5 | run_actions 2.12 | 519+18176 | 150 | 29 | 136 | 1 |  |
| 9 | 3.6 | 4.8 | run_command 0.12 | 15367+4096 | 259 | 96 | 403 | 1 |  |
| 10 | 2.6 | 3.1 | set_plan 0.00, check_done 0.12 | 1784+18688 | 126 | 24 | 82 | 1 |  |
| 11 | 4.5 | 4.7 |  | 1058+20352 | 71 | 0 | 0 | 1 |  |

model total 70.3 s, tool total 6.0 s over 11 turns; gate: 8 actions exec 3.4 s, gate decision 0.45 s, 5 observations 1.9 s (1 with screenshot), reviewer calls 0 (0.0 s)

## ow17-gimp-045bf3ff-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.5 | 3.6 | set_plan 0.00, list_apps 0.02 | 311+4992 | 136 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.2 | run_command 0.03, list_windows 0.03 | 1761+5248 | 112 | 15 | 60 | 0 |  |
| 3 | 1.8 | 4.0 | run_command 0.65, get_window_state 1.08 | 458+6912 | 311 | 102 | 386 | 0 |  |
| 4 | 1.8 | 5.4 | get_window_state 0.29 | 1568+7296 | 605 | 541 | 2070 | 1 | 17 KB image/jpeg 427x318 |
| 5 | 5.5 | 6.5 | click 0.33 | 1818+8832 | 82 | 34 | 154 | 2 | 39 KB image/jpeg 647x606 |
| 6 | 3.6 | 4.1 | get_window_state 0.48 | 176+10624 | 99 | 19 | 66 | 2 |  |
| 7 | 4.2 | 6.5 | click 0.62 | 1055+10752 | 48 | 0 | 0 | 3 | 17 KB image/jpeg 427x318 |
| 8 | 2.6 | 3.0 | get_window_state 0.61 | 168+11776 | 82 | 18 | 82 | 3 |  |
| 9 | 4.8 | 5.0 | click 1.13 | 4721+11904 | 73 | 25 | 97 | 4 | 114 KB image/jpeg 826x781 |
| 10 | 6.5 | 6.9 | run_command 0.02 | 254+16512 | 83 | 0 | 0 | 4 |  |
| 11 | 14.4 | 14.5 | get_window_state 0.97 | 311+16640 | 148 | 34 | 135 | 4 |  |
| 12 | 27.1 | 27.2 | click 0.87 | 1228+16896 | 116 | 68 | 253 | 5 | 105 KB image/jpeg 826x781 |
| 13 | 18.0 | 18.5 | get_window_state 0.60 | 260+18048 | 81 | 0 | 0 | 5 |  |
| 14 | 8.7 | 19.4 | run_command 0.02, press_key 0.15 | 2045+18304 | 1269 | 1109 | 4489 | 6 | 18 KB image/jpeg 272x543 |
| 15 | 9.1 | 9.3 | run_actions 2.35 | 1693+20224 | 428 | 244 | 840 | 6 |  |
| 16 | 10.5 | 10.5 |  | 0+0 | 0 | None | 0 | 6 |  |
| 17 | 12.2 | 16.4 | run_actions 5.79 | 3368+21888 | 738 | 532 | 2151 | 6 |  |
| 18 | 9.4 | 12.8 | click 0.85 | 1544+25216 | 509 | 460 | 1862 | 6 |  |
| 19 | 7.2 | 8.4 | run_command 0.04 | 616+26752 | 233 | 157 | 641 | 6 |  |
| 20 | 13.9 | 16.5 | run_actions 0.18 | 428+27264 | 410 | 267 | 1082 | 6 |  |
| 21 | 14.5 | 14.6 | get_window_state 0.16 | 581+27648 | 82 | 0 | 0 | 6 |  |
| 22 | 14.7 | 14.9 | run_actions 2.70 | 1507+28160 | 154 | 18 | 65 | 7 | 114 KB image/jpeg 826x781 |
| 23 | 11.4 | 12.0 | type_text 0.93 | 830+29568 | 81 | 0 | 0 | 8 | 2 KB image/jpeg 168x34 |
| 24 | 21.8 | 22.2 | get_window_state 0.31 | 213+30336 | 64 | 0 | 0 | 8 |  |
| 25 | 12.2 | 13.0 | run_actions 0.28 | 793+30464 | 109 | 0 | 0 | 9 | 7 KB image/jpeg 168x436 |
| 26 | 10.5 | 10.7 | list_windows 0.01 | 272+31232 | 45 | 0 | 0 | 9 |  |
| 27 | 15.7 | 15.8 | press_key 0.87 | 143+31488 | 246 | 167 | 709 | 9 |  |
| 28 | 6.7 | 6.9 | list_windows 0.01 | 27850+4096 | 45 | 0 | 0 | 9 |  |
| 29 | 2.5 | 3.9 | press_key 0.11 | 456+31616 | 201 | 141 | 590 | 9 |  |
| 30 | 22.1 | 22.3 |  | 344+32000 | 63 | 0 | 0 | 9 |  |

model total 337.0 s, tool total 22.5 s over 30 turns; gate: 29 actions exec 14.1 s, gate decision 0.10 s, 14 observations 4.7 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow17-gimp-045bf3ff-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 4.2 | set_plan 0.00, list_windows 0.01 | 55+5248 | 124 | 0 | 0 | 0 |  |
| 2 | 1.6 | 4.2 | get_window_state 0.32 | 448+5248 | 394 | 309 | 1167 | 0 |  |
| 3 | 5.3 | 5.5 | click 0.23 | 1417+5632 | 126 | 61 | 264 | 1 | 17 KB image/jpeg 427x318 |
| 4 | 2.1 | 2.3 | get_window_state 0.16 | 244+7040 | 93 | 11 | 44 | 1 |  |
| 5 | 2.8 | 2.9 | click 1.86 | 1390+7168 | 65 | 0 | 0 | 2 | 39 KB image/jpeg 647x606 |
| 6 | 2.3 | 2.6 | get_window_state 0.69 | 219+8448 | 132 | 34 | 144 | 2 |  |
| 7 | 6.1 | 7.4 | run_actions 2.53 | 1033+8576 | 203 | 64 | 220 | 3 | 114 KB image/jpeg 826x781 |
| 8 | 3.8 | 5.8 | run_actions 2.45 | 3060+9600 | 304 | 179 | 721 | 3 |  |
| 9 | 7.2 | 9.4 | run_actions 2.10 | 3568+12544 | 330 | 177 | 697 | 3 |  |
| 10 | 12.3 | 13.2 | list_windows 0.01 | 3602+16000 | 160 | 115 | 526 | 3 |  |
| 11 | 10.3 | 10.6 | get_window_state 0.09 | 291+19584 | 103 | 21 | 79 | 3 |  |
| 12 | 3.9 | 4.3 | click 1.54 | 632+19840 | 77 | 11 | 48 | 4 | 11 KB image/jpeg 414x275 |
| 13 | 25.9 | 26.0 | get_window_state 0.03 | 16621+4096 | 82 | 0 | 0 | 4 |  |
| 14 | 154.9 | 155.5 | get_window_state 1.63 | 827+20352 | 118 | 18 | 85 | 4 |  |
| 15 | 6.6 | 8.0 | run_actions 1.47 | 788+21120 | 260 | 118 | 515 | 5 | 12 KB image/jpeg 414x275 |
| 16 | 4.8 | 7.6 | click 0.86 | 2194+20608 | 452 | 387 | 1674 | 5 |  |
| 17 | 14.2 | 15.6 | run_actions 0.28 | 1757+21888 | 202 | 85 | 349 | 5 |  |
| 18 | 10.5 | 10.8 | list_windows 0.01 | 412+23552 | 45 | 0 | 0 | 5 |  |
| 19 | 9.0 | 9.0 | set_plan 0.00 | 157+23936 | 225 | 100 | 432 | 5 |  |
| 20 | 6.3 | 6.9 | check_done 0.02 | 371+24064 | 28 | 0 | 0 | 5 |  |
| 21 | 4.2 | 4.5 |  | 327+24320 | 77 | 0 | 0 | 5 |  |

model total 316.5 s, tool total 16.3 s over 21 turns; gate: 10 actions exec 7.1 s, gate decision 0.87 s, 15 observations 3.0 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow23-libreoffice_impress-455d3c66-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.3 | 2.6 | list_windows 0.03, set_plan 0.03 | 387+4992 | 121 | 0 | 0 | 0 |  |
| 2 | 4.8 | 4.9 | get_window_state 0.34 | 375+5376 | 118 | 37 | 150 | 0 |  |
| 3 | 5.4 | 5.9 | click 0.06 | 2893+5632 | 67 | 19 | 78 | 1 | 113 KB image/jpeg 1396x823 |
| 4 | 6.1 | 6.5 | get_window_state 0.95 | 217+8448 | 77 | 0 | 0 | 1 |  |
| 5 | 3.6 | 5.4 | press_key 0.89 | 1057+8576 | 211 | 113 | 490 | 2 | 113 KB image/jpeg 1396x823 |
| 6 | 3.5 | 3.6 | get_window_state 0.54 | 310+9600 | 77 | 0 | 0 | 2 |  |
| 7 | 3.1 | 4.5 | click 1.75 | 1022+9856 | 204 | 109 | 464 | 3 | 113 KB image/jpeg 1396x823 |
| 8 | 6.3 | 6.5 | click 1.00 | 424+10752 | 94 | 0 | 0 | 3 |  |
| 9 | 7.4 | 7.7 | get_window_state 0.47 | 225+11136 | 77 | 0 | 0 | 3 |  |
| 10 | 15.9 | 16.9 | click 0.02 | 665+11264 | 127 | 79 | 363 | 4 | 21 KB image/jpeg 312x594 |
| 11 | 14.9 | 16.5 | click 1.54 | 257+11904 | 208 | 114 | 436 | 4 |  |
| 12 | 13.8 | 15.1 | click 0.23 | 357+12160 | 244 | 131 | 544 | 4 |  |
| 13 | 15.1 | 15.6 | list_windows 0.01 | 436+12416 | 45 | 0 | 0 | 4 |  |
| 14 | 6.1 | 13.8 | get_window_state 0.16 | 195+12800 | 1016 | 934 | 3590 | 4 |  |
| 15 | 13.7 | 13.8 | press_key 0.11 | 3682+12928 | 104 | 25 | 111 | 5 | 38 KB image/jpeg 740x511 |
| 16 | 14.5 | 16.8 | click 0.84 | 270+16512 | 243 | 149 | 702 | 5 |  |
| 17 | 12.1 | 12.5 | list_windows 0.01 | 348+16768 | 45 | 0 | 0 | 5 |  |
| 18 | 9.4 | 10.0 | get_window_state 1.50 | 198+17024 | 143 | 66 | 293 | 5 |  |
| 19 | 5.4 | 9.5 | click 1.23 | 566+17152 | 454 | 341 | 1213 | 6 | 21 KB image/jpeg 312x594 |
| 20 | 4.0 | 4.3 | list_windows 0.01 | 599+17664 | 45 | 0 | 0 | 6 |  |
| 21 | 15.7 | 16.6 | get_window_state 0.24 | 219+18176 | 90 | 8 | 34 | 6 |  |
| 22 | 22.6 | 22.7 | click 1.56 | 1594+18304 | 116 | 68 | 289 | 7 | 31 KB image/jpeg 782x470 |
| 23 | 10.0 | 10.5 | press_key 0.86 | 263+19840 | 98 | 0 | 0 | 7 |  |
| 24 | 19.3 | 19.7 | type_text 0.91 | 171+20096 | 79 | 0 | 0 | 7 |  |
| 25 | 18.2 | 18.7 | click 0.71 | 189+20224 | 55 | 0 | 0 | 7 |  |
| 26 | 19.7 | 20.3 | get_window_state 0.58 | 225+20352 | 99 | 0 | 0 | 7 |  |
| 27 | 6.6 | 6.8 | click 0.69 | 551+20480 | 173 | 60 | 187 | 8 | 29 KB image/jpeg 571x324 |
| 28 | 2.4 | 2.9 | get_window_state 0.18 | 303+20992 | 82 | 0 | 0 | 8 |  |
| 29 | 5.5 | 6.0 | click 0.84 | 1543+21248 | 66 | 18 | 71 | 9 | 29 KB image/jpeg 782x470 |
| 30 | 17.5 | 17.8 |  | 180+22784 | 82 | 0 | 0 | 9 |  |

model total 334.2 s, tool total 18.3 s over 30 turns; gate: 16 actions exec 13.1 s, gate decision 0.10 s, 13 observations 4.1 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow23-libreoffice_impress-455d3c66-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.2 | set_plan 0.00, list_windows 0.01 | 3+5376 | 130 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.6 | get_window_state 0.25 | 385+5376 | 131 | 48 | 209 | 0 |  |
| 3 | 4.9 | 6.2 | get_window_state 0.52 | 858+5760 | 148 | 50 | 232 | 1 | 113 KB image/jpeg 1396x823 |
| 4 | 2.5 | 3.1 | click 1.14 | 418+6528 | 117 | 23 | 77 | 1 |  |
| 5 | 15.3 | 15.7 | click 0.93 | 7102+6912 | 128 | 80 | 314 | 3 | 113 KB image/jpeg 1396x823; 113 KB image/jpeg 1396x823 |
| 6 | 22.2 | 22.6 | click 2.46 | 3393+13952 | 59 | 11 | 35 | 4 | 114 KB image/jpeg 1396x823 |
| 7 | 23.9 | 24.7 | click 2.30 | 1523+17280 | 115 | 67 | 290 | 5 | 31 KB image/jpeg 782x470 |
| 8 | 28.7 | 29.5 | run_actions 3.35 | 627+18688 | 165 | 15 | 52 | 6 | 31 KB image/jpeg 782x470 |
| 9 | 19.6 | 20.7 | get_window_state 0.22 | 637+19200 | 153 | 89 | 333 | 6 |  |
| 10 | 30.9 | 31.0 | click 5.17 | 1195+19712 | 49 | 0 | 0 | 7 | 29 KB image/jpeg 571x324 |
| 11 | 19.4 | 21.0 | press_key 1.18 | 305+20864 | 206 | 127 | 507 | 7 |  |
| 12 | 31.3 | 32.1 | get_window_state 0.41 | 406+21120 | 120 | 39 | 184 | 7 |  |
| 13 | 22.7 | 25.3 | run_actions 6.53 | 645+21504 | 417 | 208 | 774 | 8 | 46 KB image/jpeg 782x470 |
| 14 | 3.4 | 4.1 | get_window_state 0.00 | 932+22144 | 64 | 0 | 0 | 8 |  |
| 15 | 25.6 | 26.2 | get_window_state 0.19 | 185+23040 | 89 | 9 | 34 | 8 |  |
| 16 | 14.8 | 16.0 | run_actions 2.17 | 1560+23168 | 211 | 86 | 226 | 9 | 29 KB image/jpeg 782x470 |
| 17 | 25.7 | 26.4 | run_actions 0.40 | 535+24704 | 129 | 32 | 138 | 9 |  |
| 18 | 21.7 | 22.8 | get_window_state 0.34 | 324+25216 | 97 | 0 | 0 | 9 |  |
| 19 | 21.1 | 21.5 | click 1.16 | 611+25472 | 65 | 17 | 68 | 10 | 29 KB image/jpeg 782x470 |
| 20 | 23.8 | 24.2 | click 1.05 | 1441+25984 | 54 | 0 | 0 | 10 |  |
| 21 | 21.0 | 21.2 | run_command 0.05 | 182+27392 | 61 | 0 | 0 | 10 |  |
| 22 | 21.6 | 22.7 | check_done 0.09, set_plan 0.09 | 234+27520 | 196 | 48 | 225 | 10 |  |
| 23 | 23.5 | 23.9 |  | 657+27648 | 56 | 0 | 0 | 10 |  |

model total 445.5 s, tool total 30.0 s over 23 turns; gate: 23 actions exec 18.8 s, gate decision 2.35 s, 20 observations 6.8 s (10 with screenshot), reviewer calls 0 (0.0 s)

