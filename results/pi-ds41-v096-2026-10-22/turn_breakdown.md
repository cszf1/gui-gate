# Per-turn time breakdown (bench/turn_breakdown.py)

## og01-impress-pre-pptx-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.7 | set_plan 0.00, list_windows 0.01 | 368+4992 | 124 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.2 | press_key 0.10 | 482+5248 | 128 | 27 | 99 | 0 |  |
| 3 | 3.5 | 3.6 | get_window_state 0.21 | 294+5632 | 83 | 0 | 0 | 0 |  |
| 4 | 7.8 | 8.8 | get_window_state 0.27 | 904+5888 | 181 | 99 | 426 | 1 | 120 KB image/jpeg 1396x823 |
| 5 | 5.7 | 6.5 | run_actions 4.54 | 1472+6784 | 215 | 37 | 147 | 1 |  |
| 6 | 8.5 | 10.0 | click 0.80 | 643+8192 | 227 | 162 | 728 | 1 |  |
| 7 | 2.1 | 2.4 | run_command 0.06 | 323+8832 | 75 | 0 | 0 | 1 |  |
| 8 | 4.4 | 5.5 | run_command 0.05 | 320+9088 | 205 | 105 | 447 | 1 |  |
| 9 | 4.2 | 5.2 | press_key 0.88 | 338+9344 | 191 | 65 | 288 | 1 |  |
| 10 | 10.9 | 11.4 | get_window_state 0.01 | 341+9600 | 101 | 0 | 0 | 1 |  |
| 11 | 7.5 | 7.8 | get_window_state 0.09 | 232+9856 | 100 | 0 | 0 | 1 |  |
| 12 | 2.8 | 3.9 | run_actions 0.88 | 1393+9984 | 172 | 42 | 184 | 1 |  |
| 13 | 5.3 | 6.2 | run_actions 0.02 | 389+11264 | 134 | 23 | 105 | 1 |  |
| 14 | 12.3 | 13.8 | run_actions 1.61 | 269+11648 | 115 | 0 | 0 | 1 |  |
| 15 | 3.5 | 3.9 | click 1.35 | 350+11904 | 65 | 0 | 0 | 1 |  |
| 16 | 4.6 | 6.3 | get_window_state 0.06 | 261+12160 | 115 | 15 | 61 | 1 |  |
| 17 | 9.0 | 10.1 | run_command 0.04 | 748+12416 | 152 | 39 | 184 | 1 |  |
| 18 | 3.0 | 3.3 | set_plan 0.00 | 353+13056 | 52 | 0 | 0 | 1 |  |
| 19 | 9.3 | 9.6 | check_done 0.11 | 209+13312 | 52 | 0 | 0 | 1 |  |
| 20 | 11.6 | 12.0 |  | 365+13440 | 53 | 0 | 0 | 1 |  |

model total 135.2 s, tool total 11.1 s over 20 turns; gate: 13 actions exec 8.1 s, gate decision 0.05 s, 8 observations 1.4 s (1 with screenshot), reviewer calls 0 (0.0 s)

## og01-impress-pre-pptx-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.5 | set_plan 0.00, list_windows 0.01 | 112+5248 | 133 | 0 | 0 | 0 |  |
| 2 | 1.2 | 2.3 | press_key 0.37 | 504+5248 | 206 | 89 | 347 | 0 |  |
| 3 | 1.2 | 1.8 | click 0.68 | 2781+5632 | 123 | 39 | 159 | 0 |  |
| 4 | 2.0 | 2.4 | click 2.02 | 2722+8320 | 92 | 8 | 24 | 0 |  |
| 5 | 1.2 | 4.3 | run_actions 4.35 | 1487+11008 | 503 | 336 | 1398 | 0 |  |
| 6 | 2.0 | 3.2 | get_window_state 0.26 | 925+12416 | 209 | 93 | 428 | 0 |  |
| 7 | 1.9 | 2.2 | get_window_state 0.22 | 310+13312 | 117 | 0 | 0 | 0 |  |
| 8 | 2.5 | 3.6 | get_window_state 0.30 | 367+13568 | 228 | 128 | 546 | 0 |  |
| 9 | 2.6 | 3.2 | click 0.01 | 1937+13824 | 125 | 26 | 90 | 1 | 37 KB image/jpeg 782x497 |
| 10 | 2.6 | 4.6 | run_command 0.02, run_actions 3.38 | 285+15744 | 425 | 210 | 905 | 1 |  |
| 11 | 3.3 | 6.1 | click 0.94 | 952+16000 | 471 | 386 | 1671 | 1 |  |
| 12 | 3.8 | 4.6 | run_command 0.02, list_windows 0.03 | 625+16896 | 106 | 19 | 85 | 1 |  |
| 13 | 5.7 | 7.9 | request_human 0.00 | 388+17408 | 352 | 250 | 1093 | 1 |  |
| 14 | 2.9 | 3.8 | set_plan 0.00, check_done 0.10 | 379+17792 | 176 | 19 | 73 | 1 |  |
| 15 | 1.7 | 2.3 |  | 637+18048 | 82 | 0 | 0 | 1 |  |

model total 54.9 s, tool total 12.7 s over 15 turns; gate: 14 actions exec 8.3 s, gate decision 0.76 s, 10 observations 2.9 s (1 with screenshot), reviewer calls 0 (0.0 s)

## og02-writer-pdf-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 12.6 | 12.8 | set_plan 0.00, list_windows 0.06 | 389+4992 | 126 | 0 | 0 | 0 |  |
| 2 | 1.4 | 1.9 | run_command 0.03 | 380+5376 | 93 | 22 | 92 | 0 |  |
| 3 | 1.9 | 3.4 | run_command 0.90 | 302+5632 | 117 | 26 | 115 | 0 |  |
| 4 | 2.7 | 2.8 | run_command 0.03, get_window_state 0.50 | 1648+5888 | 253 | 73 | 284 | 0 |  |
| 5 | 4.2 | 6.6 | click 0.32 | 3336+7424 | 111 | 63 | 255 | 1 | 95 KB image/jpeg 1396x823 |
| 6 | 5.2 | 6.0 | get_window_state 0.29 | 184+10752 | 98 | 0 | 0 | 1 |  |
| 7 | 10.6 | 11.5 | click 0.73 | 580+10880 | 133 | 85 | 397 | 1 |  |
| 8 | 6.6 | 6.8 | get_window_state 0.19 | 246+11392 | 100 | 0 | 0 | 1 |  |
| 9 | 9.0 | 9.5 | click 0.01 | 2912+11520 | 159 | 65 | 282 | 2 | 49 KB image/jpeg 979x499 |
| 10 | 6.5 | 7.0 | get_window_state 0.31 | 349+14336 | 82 | 0 | 0 | 2 |  |
| 11 | 6.1 | 6.3 | click 0.84 | 1626+14592 | 48 | 0 | 0 | 3 | 34 KB image/jpeg 782x444 |
| 12 | 13.8 | 14.1 | get_window_state 2.07 | 226+16128 | 82 | 0 | 0 | 3 |  |
| 13 | 4.9 | 5.5 | click 5.18 | 600+16256 | 137 | 43 | 141 | 4 | 8 KB image/jpeg 546x115 |
| 14 | 9.1 | 9.6 | run_command 2.07 | 316+16768 | 83 | 0 | 0 | 4 |  |
| 15 | 3.3 | 3.7 | list_windows 0.01 | 448+17024 | 72 | 26 | 114 | 4 |  |
| 16 | 21.1 | 22.8 | run_command 0.02 | 194+17408 | 275 | 190 | 758 | 4 |  |
| 17 | 13.9 | 16.6 | run_command 0.02 | 641+17536 | 385 | 311 | 1426 | 4 |  |
| 18 | 7.5 | 8.0 | get_window_state 1.02 | 559+18176 | 119 | 0 | 0 | 4 |  |
| 19 | 19.3 | 20.3 | get_window_state 1.08 | 400+18688 | 181 | 99 | 435 | 4 |  |
| 20 | 7.7 | 10.8 | click 1.80 | 633+19072 | 490 | 396 | 1584 | 5 | 8 KB image/jpeg 546x115 |
| 21 | 19.5 | 20.5 | run_command 3.03 | 702+19584 | 62 | 0 | 0 | 5 |  |
| 22 | 13.8 | 14.5 | get_window_state 1.07 | 349+20224 | 138 | 38 | 156 | 5 |  |
| 23 | 11.5 | 14.4 | press_key 1.33 | 667+20480 | 389 | 310 | 1229 | 6 | 8 KB image/jpeg 546x115 |
| 24 | 9.3 | 9.9 | get_window_state 1.30 | 482+21120 | 100 | 0 | 0 | 6 |  |
| 25 | 20.6 | 21.7 | list_windows 0.01 | 634+21504 | 113 | 68 | 283 | 7 | 8 KB image/jpeg 546x115 |
| 26 | 11.1 | 13.3 | click 1.82 | 343+22016 | 444 | 350 | 1110 | 7 |  |
| 27 | 2.5 | 3.1 | run_command 3.05 | 622+22272 | 62 | 0 | 0 | 7 |  |
| 28 | 4.3 | 4.6 | get_window_state 1.07 | 397+22784 | 100 | 0 | 0 | 7 |  |
| 29 | 13.0 | 16.8 | press_key 0.89 | 553+23168 | 272 | 193 | 816 | 8 | 8 KB image/jpeg 546x115 |
| 30 | 8.8 | 9.1 |  | 379+23680 | 85 | 0 | 0 | 8 |  |

model total 313.7 s, tool total 31.0 s over 30 turns; gate: 17 actions exec 21.6 s, gate decision 0.34 s, 13 observations 8.1 s (8 with screenshot), reviewer calls 0 (0.0 s)

## og02-writer-pdf-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.2 | set_plan 0.00, list_windows 0.01 | 5+5376 | 119 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.8 | run_command 0.05, get_window_state 0.12 | 1645+4096 | 221 | 83 | 354 | 0 |  |
| 3 | 10.3 | 10.6 | click 0.73 | 3174+5376 | 240 | 175 | 741 | 0 |  |
| 4 | 1.3 | 2.0 | click 1.67 | 2893+8448 | 138 | 73 | 320 | 0 |  |
| 5 | 4.5 | 4.6 | click 0.02 | 1509+11264 | 183 | 118 | 503 | 0 |  |
| 6 | 2.1 | 4.0 | run_command 0.02 | 432+12672 | 313 | 238 | 1038 | 0 |  |
| 7 | 3.1 | 4.9 | run_actions 4.13 | 596+13056 | 318 | 171 | 773 | 0 |  |
| 8 | 2.5 | 2.6 | click 0.94 | 740+13568 | 65 | 0 | 0 | 0 |  |
| 9 | 4.2 | 4.4 | run_command 0.02 | 259+14208 | 102 | 0 | 0 | 0 |  |
| 10 | 1.8 | 2.5 | set_plan 0.00, check_done 0.09 | 311+14464 | 187 | 38 | 158 | 0 |  |
| 11 | 2.5 | 3.0 |  | 568+14720 | 93 | 0 | 0 | 0 |  |

model total 43.6 s, tool total 7.8 s over 11 turns; gate: 10 actions exec 5.2 s, gate decision 0.67 s, 5 observations 1.5 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og03-calc-csv-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.0 | 3.8 | set_plan 0.00 | 483+4992 | 158 | 49 | 221 | 0 |  |
| 2 | 2.1 | 2.4 | list_windows 0.02, list_apps 0.04 | 332+5376 | 46 | 0 | 0 | 0 |  |
| 3 | 2.9 | 2.9 | get_window_state 1.29 | 1864+5632 | 115 | 15 | 80 | 0 |  |
| 4 | 4.6 | 7.4 | click 0.30 | 4361+7424 | 388 | 323 | 1391 | 1 | 118 KB image/jpeg 1396x823 |
| 5 | 7.9 | 8.4 | get_window_state 1.21 | 8146+4096 | 116 | 0 | 0 | 1 |  |
| 6 | 16.3 | 17.0 | click 0.78 | 1833+11776 | 103 | 38 | 158 | 2 | 112 KB image/jpeg 1396x823 |
| 7 | 7.3 | 7.4 | get_window_state 1.59 | 239+13568 | 100 | 0 | 0 | 2 |  |
| 8 | 19.0 | 19.7 | click 1.39 | 2915+13696 | 110 | 45 | 193 | 3 | 49 KB image/jpeg 782x497 |
| 9 | 9.2 | 9.3 | get_window_state 1.84 | 322+16512 | 117 | 0 | 0 | 3 |  |
| 10 | 17.3 | 18.2 | click 1.39 | 632+16768 | 93 | 9 | 34 | 4 | 29 KB image/jpeg 588x347 |
| 11 | 24.7 | 26.5 | get_window_state 0.55 | 350+17280 | 231 | 132 | 503 | 4 |  |
| 12 | 20.6 | 23.5 | run_actions 3.27 | 714+17536 | 507 | 358 | 1244 | 5 | 29 KB image/jpeg 588x347 |
| 13 | 25.6 | 27.1 | get_window_state 0.60 | 1138+18176 | 256 | 134 | 611 | 5 |  |
| 14 | 10.1 | 10.6 | click 0.78 | 1064+19200 | 72 | 7 | 22 | 6 | 47 KB image/jpeg 782x497 |
| 15 | 5.6 | 6.1 | get_window_state 0.19 | 208+20224 | 107 | 7 | 35 | 6 |  |
| 16 | 10.7 | 11.3 | click 0.75 | 654+20352 | 115 | 31 | 155 | 7 | 6 KB image/jpeg 422x115 |
| 17 | 16.2 | 16.4 | list_windows 0.01 | 9065+12160 | 56 | 10 | 52 | 7 |  |
| 18 | 27.4 | 28.6 | get_window_state 0.45 | 348+20992 | 212 | 94 | 427 | 7 |  |
| 19 | 31.7 | 32.7 | get_window_state 0.66 | 1208+21248 | 154 | 54 | 270 | 8 | 119 KB image/jpeg 1396x823 |
| 20 | 35.1 | 35.5 | click 1.44 | 1125+22400 | 88 | 3 | 9 | 9 | 15 KB image/jpeg 480x307 |
| 21 | 41.0 | 41.1 | run_command 0.04, list_windows 0.04 | 282+23424 | 150 | 10 | 55 | 9 |  |
| 22 | 33.1 | 34.1 | get_window_state 1.00 | 351+23680 | 166 | 50 | 211 | 9 |  |
| 23 | 45.0 | 46.9 | get_window_state 0.91 | 994+23936 | 372 | 272 | 1280 | 10 | 64 KB image/jpeg 979x499 |
| 24 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 437.0 s, tool total 20.6 s over 24 turns; gate: 12 actions exec 9.4 s, gate decision 0.12 s, 15 observations 9.8 s (11 with screenshot), reviewer calls 0 (0.0 s)

## og03-calc-csv-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.6 | set_plan 0.01, list_windows 0.03 | 99+5376 | 145 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.1 | run_command 0.03 | 517+5376 | 81 | 10 | 48 | 0 |  |
| 3 | 1.5 | 1.9 | run_command 8.12 | 173+5888 | 93 | 15 | 65 | 0 |  |
| 4 | 3.1 | 4.2 | run_command 0.03, get_window_state 1.37 | 1023+6016 | 230 | 91 | 372 | 0 |  |
| 5 | 10.8 | 12.9 | run_command 0.03 | 4730+6912 | 327 | 226 | 992 | 1 | 118 KB image/jpeg 1396x823 |
| 6 | 12.2 | 13.3 | set_plan 0.00, press_key 1.06 | 1353+11520 | 246 | 109 | 444 | 1 |  |
| 7 | 12.8 | 14.1 | click 1.22 | 2846+12800 | 269 | 221 | 903 | 1 |  |
| 8 | 9.1 | 9.8 | get_window_state 0.92 | 493+15616 | 101 | 19 | 71 | 1 |  |
| 9 | 14.5 | 14.6 | click 5.96 | 1127+16000 | 48 | 0 | 0 | 2 | 28 KB image/jpeg 588x347 |
| 10 | 13.8 | 15.9 | press_key 1.74 | 371+17024 | 309 | 230 | 879 | 2 |  |
| 11 | 4.5 | 6.0 | get_window_state 0.09 | 575+17280 | 125 | 24 | 110 | 2 |  |
| 12 | 12.9 | 14.5 | run_actions 7.30 | 530+17792 | 343 | 126 | 352 | 3 | 46 KB image/jpeg 782x497 |
| 13 | 15.6 | 16.0 | click 0.03 | 1078+18304 | 78 | 30 | 125 | 3 |  |
| 14 | 16.7 | 18.0 | request_human 0.00 | 282+19328 | 395 | 274 | 1253 | 3 |  |
| 15 | 17.4 | 18.4 | get_window_state 0.10 | 444+19584 | 191 | 90 | 330 | 3 |  |
| 16 | 9.4 | 10.9 | click 1.93 | 540+19968 | 214 | 166 | 642 | 4 | 47 KB image/jpeg 782x497 |
| 17 | 19.0 | 19.7 | run_actions 3.24 | 416+20480 | 196 | 0 | 0 | 4 |  |
| 18 | 20.7 | 22.8 | click 0.91 | 1060+20864 | 300 | 251 | 1049 | 4 |  |
| 19 | 15.1 | 15.3 | run_actions 1.18 | 435+21888 | 135 | 0 | 0 | 4 |  |
| 20 | 9.8 | 12.0 | run_actions 5.03 | 2676+22272 | 385 | 183 | 685 | 4 |  |
| 21 | 12.7 | 15.5 | run_actions 0.01 | 880+24832 | 412 | 120 | 468 | 4 |  |
| 22 | 16.0 | 17.5 | run_actions 1.34 | 602+25600 | 343 | 52 | 226 | 4 |  |
| 23 | 7.1 | 8.1 | get_window_state 0.12 | 529+26112 | 176 | 75 | 330 | 4 |  |
| 24 | 22.4 | 22.5 | type_text 2.14 | 482+26624 | 151 | 32 | 145 | 5 | 48 KB image/jpeg 782x497 |
| 25 | 6.5 | 6.9 | get_window_state 0.09 | 320+27008 | 101 | 0 | 0 | 5 |  |
| 26 | 16.5 | 17.9 | run_actions 8.30 | 454+27264 | 282 | 36 | 114 | 6 | 49 KB image/jpeg 782x497 |
| 27 | 30.3 | 30.8 | click 1.47 | 1217+27648 | 88 | 40 | 155 | 6 |  |
| 28 | 27.3 | 27.8 | get_window_state 0.07 | 885+28800 | 101 | 0 | 0 | 6 |  |
| 29 | 28.1 | 28.3 | click 0.91 | 465+29568 | 70 | 22 | 80 | 7 | 16 KB image/jpeg 480x307 |
| 30 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 420.4 s, tool total 54.8 s over 30 turns; gate: 40 actions exec 39.4 s, gate decision 1.83 s, 19 observations 9.4 s (7 with screenshot), reviewer calls 0 (0.0 s)

## og04-os-copy-dir2-conflict-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.8 | set_plan 0.00, run_command 0.02 | 76+5248 | 208 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.1 | run_command 0.08 | 436+5248 | 115 | 20 | 99 | 0 |  |
| 3 | 2.5 | 3.1 | run_command 0.02 | 1093+5632 | 80 | 14 | 53 | 0 |  |
| 4 | 1.7 | 2.6 | run_command 0.02 | 446+6656 | 188 | 119 | 461 | 0 |  |
| 5 | 3.1 | 7.0 | run_command 0.03 | 340+7040 | 713 | 591 | 2449 | 0 |  |
| 6 | 2.0 | 3.0 | set_plan 0.00, check_done 0.02 | 1122+7296 | 113 | 0 | 0 | 0 |  |
| 7 | 2.3 | 3.1 |  | 579+8320 | 90 | 0 | 0 | 0 |  |

model total 23.7 s, tool total 0.2 s over 7 turns; gate: 5 actions exec 0.2 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og04-os-copy-dir2-conflict-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.5 | 4.0 | set_plan 0.01, run_command 0.03 | 76+5248 | 177 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.1 | run_command 0.02 | 514+5248 | 69 | 13 | 58 | 0 |  |
| 3 | 1.5 | 4.0 | run_command 0.01 | 317+5760 | 430 | 296 | 1188 | 0 |  |
| 4 | 1.9 | 3.3 | run_command 0.03 | 628+6016 | 155 | 38 | 147 | 0 |  |
| 5 | 1.2 | 1.9 | set_plan 0.00, check_done 0.02 | 621+6528 | 114 | 0 | 0 | 0 |  |
| 6 | 1.5 | 2.0 |  | 584+7040 | 82 | 0 | 0 | 0 |  |

model total 17.3 s, tool total 0.1 s over 6 turns; gate: 4 actions exec 0.1 s, gate decision 0.01 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og05-os-fails-keep-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 12.3 | 12.4 | set_plan 0.00, run_command 0.02 | 79+5248 | 125 | 0 | 0 | 0 |  |
| 2 | 1.9 | 3.8 | run_command 0.34 | 1089+5248 | 54 | 0 | 0 | 0 |  |
| 3 | 1.5 | 2.2 | run_command 0.05 | 226+6272 | 100 | 16 | 68 | 0 |  |
| 4 | 1.4 | 1.9 | run_command 0.02 | 2809+4096 | 133 | 57 | 228 | 0 |  |
| 5 | 2.0 | 2.7 | run_command 0.02 | 871+6400 | 155 | 54 | 232 | 0 |  |
| 6 | 2.6 | 3.3 | set_plan 0.00, check_done 0.02 | 414+7168 | 143 | 0 | 0 | 0 |  |
| 7 | 3.8 | 6.9 | run_command 0.04 | 569+7552 | 563 | 424 | 1833 | 0 |  |
| 8 | 2.5 | 4.0 | run_command 0.02 | 912+8064 | 150 | 62 | 255 | 0 |  |
| 9 | 10.1 | 10.7 | check_done 0.02 | 306+8960 | 101 | 0 | 0 | 0 |  |
| 10 | 1.9 | 2.6 |  | 490+9216 | 114 | 0 | 0 | 0 |  |

model total 50.6 s, tool total 0.6 s over 10 turns; gate: 7 actions exec 0.5 s, gate decision 0.02 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og05-os-fails-keep-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.9 | set_plan 0.00, list_windows 0.01 | 79+5248 | 188 | 83 | 367 | 0 |  |
| 2 | 2.7 | 2.9 | run_command 0.02 | 497+5248 | 82 | 17 | 62 | 0 |  |
| 3 | 2.0 | 3.8 | run_command 0.04 | 1069+4864 | 296 | 195 | 635 | 0 |  |
| 4 | 1.4 | 2.2 | run_command 0.02 | 762+5632 | 164 | 38 | 163 | 0 |  |
| 5 | 2.0 | 2.4 | set_plan 0.00, check_done 0.02 | 555+6272 | 186 | 43 | 168 | 0 |  |
| 6 | 1.8 | 1.9 |  | 695+6784 | 99 | 24 | 84 | 0 |  |

model total 16.1 s, tool total 0.1 s over 6 turns; gate: 3 actions exec 0.1 s, gate decision 0.01 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og06-os-rename-hidden-sentinel-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 6.0 | 6.8 | set_plan 0.01, run_command 0.05 | 1236+4096 | 179 | 0 | 0 | 0 |  |
| 2 | 1.9 | 4.0 | run_command 2.10, run_command 2.12 | 409+5248 | 411 | 256 | 1032 | 0 |  |
| 3 | 2.7 | 3.3 | run_command 0.02 | 1284+5632 | 122 | 23 | 82 | 0 |  |
| 4 | 2.5 | 3.1 | run_command 0.02 | 367+6912 | 156 | 64 | 252 | 0 |  |
| 5 | 1.2 | 1.6 | set_plan 0.00, check_done 0.10 | 3548+4096 | 102 | 0 | 0 | 0 |  |
| 6 | 2.7 | 2.9 |  | 797+7168 | 45 | 0 | 0 | 0 |  |

model total 21.8 s, tool total 4.4 s over 6 turns; gate: 5 actions exec 2.2 s, gate decision 0.03 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og06-os-rename-hidden-sentinel-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.1 | 3.2 | set_plan 0.00, run_command 0.03 | 84+5248 | 182 | 0 | 0 | 0 |  |
| 2 | 3.2 | 3.8 | run_command 1.07 | 1120+5248 | 124 | 14 | 58 | 0 |  |
| 3 | 2.3 | 2.8 | run_command 0.02 | 296+6272 | 142 | 68 | 256 | 0 |  |
| 4 | 2.3 | 2.8 | run_command 0.04 | 346+6528 | 178 | 80 | 365 | 0 |  |
| 5 | 1.7 | 3.7 | run_command 0.02 | 439+6784 | 91 | 0 | 0 | 0 |  |
| 6 | 1.8 | 2.3 | set_plan 0.00, check_done 0.01 | 388+7168 | 102 | 0 | 0 | 0 |  |
| 7 | 14.3 | 14.3 |  | 0+0 | 0 | None | 0 | 0 |  |
| 8 | 1.8 | 2.8 |  | 382+7552 | 45 | 0 | 0 | 0 |  |

model total 35.8 s, tool total 1.2 s over 8 turns; gate: 5 actions exec 1.2 s, gate decision 0.02 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og06-os-rename-hidden-sentinel-gate8-s0-try2

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.2 | 1.2 |  | 0+0 | 0 | None | 0 | 0 |  |
| 2 | 0.8 | 0.8 |  | 0+0 | 0 | None | 0 | 0 |  |
| 3 | 2.1 | 2.1 |  | 0+0 | 0 | None | 0 | 0 |  |
| 4 | 1.1 | 1.1 |  | 0+0 | 0 | None | 0 | 0 |  |

model total 5.1 s, tool total 0.0 s over 4 turns; gate: 0 actions exec 0.0 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og06-os-rename-hidden-sentinel-gate8-s0-try3

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 4.4 | set_plan 0.01, run_command 0.03 | 468+4864 | 167 | 0 | 0 | 0 |  |
| 2 | 2.6 | 4.0 | run_command 0.02 | 391+5248 | 119 | 40 | 161 | 0 |  |
| 3 | 1.0 | 1.6 | run_command 0.03, list_windows 0.04 | 917+5632 | 125 | 34 | 124 | 0 |  |
| 4 | 1.5 | 2.2 | run_command 0.03 | 470+6528 | 129 | 25 | 96 | 0 |  |
| 5 | 1.3 | 1.9 | run_command 0.02 | 356+6912 | 83 | 0 | 0 | 0 |  |
| 6 | 3.1 | 3.2 | set_plan 0.00, check_done 0.02 | 335+7168 | 99 | 0 | 0 | 0 |  |
| 7 | 1.3 | 1.6 |  | 431+7424 | 40 | 0 | 0 | 0 |  |

model total 18.9 s, tool total 0.2 s over 7 turns; gate: 5 actions exec 0.1 s, gate decision 0.01 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og07-os-chmod-sentinel-dirs-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 18.0 | 18.1 | set_plan 0.00, list_windows 0.01 | 69+5248 | 128 | 0 | 0 | 0 |  |
| 2 | 2.1 | 2.7 | run_command 0.02 | 421+5248 | 118 | 20 | 76 | 0 |  |
| 3 | 1.2 | 1.6 | run_command 0.02 | 279+5632 | 101 | 14 | 60 | 0 |  |
| 4 | 1.4 | 1.8 | check_done 0.02, set_plan 0.02 | 300+5888 | 94 | 0 | 0 | 0 |  |
| 5 | 1.8 | 2.1 |  | 438+6144 | 49 | 0 | 0 | 0 |  |

model total 26.5 s, tool total 0.1 s over 5 turns; gate: 2 actions exec 0.0 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og07-os-chmod-sentinel-dirs-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.8 | 4.1 | set_plan 0.00, list_windows 0.01 | 69+5248 | 130 | 0 | 0 | 0 |  |
| 2 | 1.3 | 1.9 | run_command 0.03 | 424+5248 | 149 | 22 | 91 | 0 |  |
| 3 | 4.7 | 4.8 | set_plan 0.00, run_command 0.02 | 347+5632 | 166 | 17 | 76 | 0 |  |
| 4 | 6.1 | 6.8 | check_done 0.02, set_plan 0.02 | 495+5888 | 183 | 26 | 111 | 0 |  |
| 5 | 6.7 | 7.8 |  | 708+6272 | 211 | 124 | 492 | 0 |  |

model total 25.3 s, tool total 0.1 s over 5 turns; gate: 2 actions exec 0.0 s, gate decision 0.00 s, 1 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og08-writer-decoy-front-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5.4 | 7.1 | set_plan 0.00, list_windows 0.01 | 6+5376 | 119 | 0 | 0 | 0 |  |
| 2 | 1.5 | 2.3 | get_window_state 0.22 | 406+5376 | 149 | 66 | 272 | 0 |  |
| 3 | 9.8 | 13.9 | click 1.81 | 897+5760 | 615 | 521 | 2102 | 1 | 105 KB image/jpeg 1396x823 |
| 4 | 2.9 | 3.5 | press_key 0.89 | 3264+4096 | 140 | 42 | 161 | 1 |  |
| 5 | 2.2 | 2.7 | click 1.48 | 907+6656 | 113 | 0 | 0 | 1 |  |
| 6 | 2.1 | 2.6 | get_window_state 0.21 | 212+7552 | 111 | 28 | 89 | 1 |  |
| 7 | 3.4 | 4.3 | press_key 0.16 | 933+7680 | 169 | 88 | 343 | 2 | 108 KB image/jpeg 1396x823 |
| 8 | 3.4 | 3.6 | press_key 0.11 | 269+8576 | 79 | 0 | 0 | 2 |  |
| 9 | 7.0 | 7.5 | get_window_state 0.20 | 155+8832 | 83 | 0 | 0 | 2 |  |
| 10 | 21.0 | 22.3 | get_window_state 0.15 | 954+8960 | 249 | 149 | 525 | 3 | 110 KB image/jpeg 1396x823 |
| 11 | 26.1 | 27.6 | click 1.30 | 3086+9856 | 268 | 220 | 930 | 3 |  |
| 12 | 6.7 | 7.9 | press_key 0.10 | 6037+7296 | 250 | 171 | 759 | 3 |  |
| 13 | 30.3 | 30.5 | press_key 0.86 | 718+12928 | 79 | 0 | 0 | 3 |  |
| 14 | 30.2 | 31.3 | run_actions 0.34 | 220+13568 | 208 | 66 | 298 | 3 |  |
| 15 | 36.6 | 41.6 | press_key 0.11 | 377+13696 | 949 | 868 | 3383 | 3 |  |
| 16 | 4.5 | 5.2 | run_actions 3.25 | 1133+13952 | 193 | 0 | 0 | 3 |  |
| 17 | 9.0 | 9.1 | get_window_state 0.20 | 2513+14976 | 105 | 22 | 105 | 3 |  |
| 18 | 11.7 | 12.3 | press_key 0.12 | 988+17408 | 88 | 9 | 43 | 4 | 107 KB image/jpeg 1396x823 |
| 19 | 11.7 | 11.9 | get_window_state 0.18 | 243+18304 | 83 | 0 | 0 | 4 |  |
| 20 | 14.5 | 15.4 | get_window_state 0.54 | 1007+18432 | 173 | 57 | 217 | 5 | 87 KB image/jpeg 1396x823 |
| 21 | 16.6 | 17.3 | get_window_state 0.29 | 397+19328 | 146 | 29 | 117 | 5 |  |
| 22 | 13.7 | 15.9 | get_window_state 0.09 | 2551+19712 | 356 | 273 | 1197 | 5 |  |
| 23 | 14.7 | 15.4 | click 0.80 | 869+22144 | 122 | 28 | 95 | 6 | 40 KB image/jpeg 920x607 |
| 24 | 13.4 | 13.8 | get_window_state 0.10 | 316+22912 | 83 | 0 | 0 | 6 |  |
| 25 | 16.5 | 17.2 | click 0.77 | 590+23168 | 145 | 51 | 173 | 7 | 44 KB image/jpeg 920x607 |
| 26 | 13.7 | 14.1 | get_window_state 0.10 | 311+23680 | 83 | 0 | 0 | 7 |  |
| 27 | 16.2 | 16.8 | click 1.04 | 532+23936 | 117 | 23 | 91 | 8 | 40 KB image/jpeg 920x607 |
| 28 | 22.8 | 23.1 | get_window_state 0.19 | 226+24448 | 83 | 0 | 0 | 8 |  |
| 29 | 16.5 | 16.8 | press_key 0.88 | 930+24576 | 121 | 23 | 108 | 9 | 107 KB image/jpeg 1396x823 |
| 30 | 19.6 | 20.3 |  | 218+25472 | 83 | 0 | 0 | 9 |  |

model total 433.4 s, tool total 16.5 s over 30 turns; gate: 21 actions exec 13.0 s, gate decision 0.11 s, 14 observations 2.1 s (9 with screenshot), reviewer calls 0 (0.0 s)

## og08-writer-decoy-front-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.1 | 2.6 | set_plan 0.00, list_windows 0.01 | 6+5376 | 114 | 0 | 0 | 0 |  |
| 2 | 7.7 | 7.9 | get_window_state 0.06 | 397+5376 | 148 | 48 | 196 | 0 |  |
| 3 | 2.4 | 3.2 | list_windows 0.01 | 1854+5760 | 120 | 72 | 315 | 0 |  |
| 4 | 1.4 | 2.2 | run_command 0.03 | 278+7552 | 146 | 76 | 334 | 0 |  |
| 5 | 1.2 | 2.3 | run_command 0.02 | 483+7808 | 173 | 106 | 474 | 0 |  |
| 6 | 2.2 | 3.0 | get_window_state 0.32 | 356+8192 | 152 | 36 | 145 | 0 |  |
| 7 | 2.6 | 4.0 | run_command 0.05 | 1317+8448 | 241 | 120 | 499 | 0 |  |
| 8 | 6.1 | 6.8 | run_command 0.04 | 1356+9728 | 257 | 29 | 122 | 0 |  |
| 9 | 2.2 | 2.9 | run_command 0.04 | 1980+11008 | 166 | 19 | 97 | 0 |  |
| 10 | 2.9 | 17.8 | run_command 1.03 | 1789+12928 | 2252 | 2169 | 9005 | 0 |  |
| 11 | 1.9 | 2.3 | get_window_state 0.20 | 2586+14592 | 156 | 38 | 112 | 0 |  |
| 12 | 2.0 | 4.6 | run_actions 5.39 | 1103+17152 | 501 | 294 | 1197 | 1 | 107 KB image/jpeg 1396x823 |
| 13 | 13.3 | 13.8 | get_window_state 0.17 | 3804+18176 | 97 | 14 | 69 | 1 |  |
| 14 | 21.3 | 22.0 | run_actions 1.08 | 938+21888 | 162 | 41 | 171 | 2 | 108 KB image/jpeg 1396x823 |
| 15 | 21.1 | 22.0 | click 0.93 | 2215+22784 | 150 | 102 | 358 | 2 |  |
| 16 | 19.2 | 19.5 | get_window_state 0.18 | 3323+24960 | 106 | 23 | 117 | 2 |  |
| 17 | 5.6 | 6.4 | press_key 2.93 | 25042+4096 | 149 | 51 | 225 | 3 | 107 KB image/jpeg 1396x823 |
| 18 | 35.0 | 36.2 | run_command 0.04 | 4165+28160 | 238 | 22 | 105 | 3 |  |
| 19 | 31.4 | 31.5 | run_command 0.05 | 391+32256 | 146 | 26 | 147 | 3 |  |
| 20 | 4.2 | 4.5 | run_command 0.04 | 761+32640 | 238 | 30 | 105 | 3 |  |
| 21 | 3.1 | 4.2 | check_done 0.22, set_plan 0.22 | 576+33280 | 162 | 59 | 213 | 3 |  |
| 22 | 3.1 | 3.6 |  | 599+33792 | 66 | 0 | 0 | 3 |  |

model total 223.2 s, tool total 13.1 s over 22 turns; gate: 18 actions exec 8.1 s, gate decision 1.55 s, 11 observations 2.6 s (3 with screenshot), reviewer calls 0 (0.0 s)

## og09-calc-decoy-front-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.5 | 3.3 | set_plan 0.00, list_windows 0.01 | 462+4992 | 134 | 0 | 0 | 0 |  |
| 2 | 3.6 | 4.2 | get_window_state 0.28 | 510+5376 | 132 | 32 | 115 | 0 |  |
| 3 | 2.0 | 3.2 | run_command 0.15 | 4129+5760 | 243 | 115 | 454 | 0 |  |
| 4 | 3.2 | 7.0 | run_command 0.10 | 472+9856 | 646 | 527 | 2008 | 0 |  |
| 5 | 1.7 | 2.5 | run_actions 1.79 | 823+10240 | 175 | 53 | 209 | 0 |  |
| 6 | 4.1 | 5.0 | get_window_state 0.52 | 3093+11008 | 187 | 90 | 383 | 0 |  |
| 7 | 7.1 | 9.2 | click 0.93 | 1721+14080 | 100 | 52 | 195 | 1 | 136 KB image/jpeg 1396x823 |
| 8 | 14.2 | 14.7 | get_window_state 0.45 | 268+15744 | 98 | 0 | 0 | 1 |  |
| 9 | 17.8 | 18.0 | click 0.02 | 971+16000 | 48 | 0 | 0 | 2 | 28 KB image/jpeg 382x693 |
| 10 | 18.7 | 28.2 | run_actions 0.59 | 229+16896 | 1344 | 1220 | 5014 | 2 |  |
| 11 | 26.6 | 27.6 | run_command 0.13 | 4361+17024 | 626 | 315 | 1333 | 2 |  |
| 12 | 20.0 | 20.9 | run_actions 1.42 | 856+21376 | 133 | 0 | 0 | 2 |  |
| 13 | 16.6 | 17.5 | click 0.02 | 2794+22144 | 48 | 0 | 0 | 2 |  |
| 14 | 14.8 | 15.8 | click 0.02 | 20995+4096 | 201 | 107 | 296 | 2 |  |
| 15 | 2.9 | 3.5 | get_window_state 0.20 | 556+24832 | 117 | 0 | 0 | 2 |  |
| 16 | 24.4 | 27.4 | click 1.38 | 502+25344 | 127 | 33 | 88 | 3 | 23 KB image/jpeg 314x619 |
| 17 | 23.9 | 27.5 | run_actions 5.78 | 389+25728 | 729 | 464 | 1836 | 3 |  |
| 18 | 21.1 | 27.4 | run_actions 1.14 | 1377+26112 | 1056 | 973 | 4207 | 3 |  |
| 19 | 19.6 | 20.6 | press_key 0.32 | 1271+27392 | 159 | 61 | 274 | 3 |  |
| 20 | 25.4 | 25.7 | list_windows 0.01 | 348+28544 | 46 | 0 | 0 | 3 |  |
| 21 | 20.6 | 21.6 | run_command 6.02 | 194+28800 | 167 | 84 | 338 | 3 |  |
| 22 | 20.0 | 20.4 | list_windows 0.01 | 295+28928 | 46 | 0 | 0 | 3 |  |
| 23 | 7.3 | 7.3 | get_window_state 0.35 | 179+29184 | 118 | 0 | 0 | 3 |  |
| 24 | 3.7 | 4.8 | click 1.67 | 3210+29312 | 196 | 102 | 436 | 4 | 132 KB image/jpeg 1396x823 |
| 25 | 32.2 | 32.8 | get_window_state 0.32 | 305+32512 | 118 | 0 | 0 | 4 |  |
| 26 | 49.8 | 51.0 | run_actions 1.67 | 2270+32768 | 221 | 116 | 470 | 5 | 116 KB image/jpeg 1396x823 |
| 27 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 446.9 s, tool total 25.3 s over 27 turns; gate: 30 actions exec 19.3 s, gate decision 0.13 s, 14 observations 4.8 s (6 with screenshot), reviewer calls 0 (0.0 s)

## og09-calc-decoy-front-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.3 | set_plan 0.00, list_windows 0.01 | 78+5376 | 134 | 0 | 0 | 0 |  |
| 2 | 1.5 | 2.0 | get_window_state 0.27 | 382+5504 | 130 | 30 | 108 | 0 |  |
| 3 | 1.7 | 7.4 | click 0.88 | 4127+5760 | 1120 | 1072 | 3262 | 0 |  |
| 4 | 3.4 | 4.9 | press_key 2.98 | 3767+9856 | 253 | 193 | 807 | 0 |  |
| 5 | 2.7 | 3.7 | get_window_state 0.30 | 497+13568 | 205 | 88 | 386 | 0 |  |
| 6 | 1.8 | 4.1 | run_actions 2.67 | 816+13952 | 438 | 300 | 1225 | 0 |  |
| 7 | 3.0 | 6.5 | press_key 2.37 | 847+14720 | 544 | 445 | 1902 | 0 |  |
| 8 | 7.1 | 7.9 | get_window_state 0.01 | 3924+15488 | 137 | 37 | 153 | 0 |  |
| 9 | 4.1 | 4.2 | list_windows 0.01 | 257+19328 | 45 | 0 | 0 | 0 |  |
| 10 | 2.3 | 3.7 | run_actions 0.28 | 168+19584 | 271 | 166 | 682 | 0 |  |
| 11 | 4.3 | 5.4 | get_window_state 0.38 | 406+19712 | 188 | 88 | 403 | 0 |  |
| 12 | 18.9 | 21.8 | click 2.44 | 2586+20096 | 480 | 386 | 1412 | 1 | 136 KB image/jpeg 1396x823 |
| 13 | 21.0 | 24.1 | run_actions 4.34 | 4103+22656 | 726 | 356 | 1260 | 2 | 123 KB image/jpeg 1396x823 |
| 14 | 23.7 | 25.5 | run_actions 4.17 | 4277+26752 | 360 | 32 | 93 | 2 |  |
| 15 | 34.5 | 35.4 | press_key 2.50 | 3861+30976 | 199 | 101 | 421 | 2 |  |
| 16 | 19.0 | 19.0 |  | 0+0 | 0 | None | 0 | 2 |  |
| 17 | 26.4 | 27.1 | run_command 0.13 | 3059+34816 | 161 | 29 | 141 | 2 |  |
| 18 | 29.2 | 30.5 | check_done 0.26, set_plan 0.26 | 522+37760 | 194 | 32 | 117 | 2 |  |
| 19 | 25.3 | 26.8 |  | 1021+38272 | 80 | 0 | 0 | 2 |  |

model total 262.3 s, tool total 24.3 s over 19 turns; gate: 37 actions exec 11.1 s, gate decision 3.19 s, 14 observations 7.5 s (2 with screenshot), reviewer calls 0 (0.0 s)

## og09-calc-decoy-front-gate8-s0-try2

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 4.1 | 4.3 | set_plan 0.00, list_apps 0.06 | 78+5376 | 127 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.1 | get_window_state 0.31 | 508+5376 | 63 | 0 | 0 | 0 |  |
| 3 | 1.7 | 3.0 | run_command 0.13 | 4258+5760 | 277 | 131 | 559 | 1 | 2 KB image/jpeg 124x53 |
| 4 | 2.9 | 5.3 | click 0.91 | 533+9984 | 440 | 392 | 1390 | 1 |  |
| 5 | 4.2 | 5.5 | list_windows 0.01 | 3077+10496 | 168 | 140 | 599 | 1 |  |
| 6 | 5.0 | 6.7 | press_key 2.23 | 399+13568 | 290 | 211 | 909 | 1 |  |
| 7 | 5.8 | 6.5 | click 0.88 | 3529+13952 | 162 | 114 | 505 | 1 |  |
| 8 | 5.3 | 6.2 | press_key 1.53 | 3009+17408 | 187 | 88 | 362 | 1 |  |
| 9 | 6.3 | 6.6 | click 1.00 | 817+20352 | 67 | 19 | 77 | 1 |  |
| 10 | 7.6 | 7.7 | click 2.40 | 239+21120 | 62 | 14 | 53 | 1 |  |
| 11 | 7.4 | 8.5 | press_key 2.22 | 270+21248 | 190 | 92 | 341 | 1 |  |
| 12 | 8.9 | 10.2 | click 1.46 | 20444+4096 | 216 | 168 | 641 | 1 |  |
| 13 | 2.5 | 2.7 | click 0.95 | 3547+21504 | 59 | 0 | 0 | 1 |  |
| 14 | 1.9 | 2.2 | get_window_state 0.47 | 247+24960 | 99 | 0 | 0 | 1 |  |
| 15 | 13.1 | 16.0 | run_actions 15.18 | 1270+25088 | 659 | 282 | 1029 | 2 | 123 KB image/jpeg 1396x823 |
| 16 | 16.6 | 17.9 | run_actions 11.62 | 4350+26240 | 355 | 55 | 139 | 2 |  |
| 17 | 22.5 | 23.7 | press_key 1.42, run_command 1.52 | 3852+30464 | 391 | 184 | 720 | 2 |  |
| 18 | 16.9 | 17.1 | list_windows 0.01 | 3327+34304 | 114 | 66 | 293 | 2 |  |
| 19 | 17.5 | 18.5 | run_command 0.10 | 335+37504 | 144 | 28 | 119 | 2 |  |
| 20 | 3.4 | 3.8 | set_plan 0.00, check_done 0.23 | 392+37760 | 144 | 46 | 214 | 2 |  |
| 21 | 13.8 | 14.3 |  | 899+38144 | 87 | 0 | 0 | 2 |  |

model total 188.7 s, tool total 44.6 s over 21 turns; gate: 41 actions exec 30.2 s, gate decision 5.43 s, 13 observations 6.3 s (2 with screenshot), reviewer calls 1 (0.7 s)

## og10-clone-dest-exists-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.4 | set_plan 0.00, run_command 0.04 | 80+5248 | 188 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.2 | run_command 0.50, run_command 0.52 | 438+5248 | 196 | 11 | 54 | 0 |  |
| 3 | 1.8 | 2.9 | run_command 0.02 | 1349+5632 | 239 | 171 | 714 | 0 |  |
| 4 | 3.5 | 5.2 | run_command 4.76 | 497+6912 | 334 | 175 | 752 | 0 |  |
| 5 | 1.6 | 2.2 | run_command 0.02 | 564+7296 | 109 | 21 | 93 | 0 |  |
| 6 | 1.6 | 2.5 | run_command 0.36 | 274+7808 | 182 | 85 | 281 | 0 |  |
| 7 | 2.4 | 3.1 | run_command 0.50 | 314+8064 | 143 | 0 | 0 | 0 |  |
| 8 | 3.0 | 4.2 | set_plan 0.00, run_command 0.04 | 363+8320 | 213 | 103 | 419 | 0 |  |
| 9 | 2.7 | 3.1 | check_done 0.02 | 449+8576 | 53 | 0 | 0 | 0 |  |
| 10 | 3.4 | 3.9 |  | 402+8960 | 80 | 0 | 0 | 0 |  |

model total 31.8 s, tool total 6.8 s over 10 turns; gate: 9 actions exec 6.3 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## og10-clone-dest-exists-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.6 | set_plan 0.00, run_command 0.02 | 80+5248 | 135 | 0 | 0 | 0 |  |
| 2 | 3.1 | 3.4 | run_command 0.02 | 361+5248 | 73 | 25 | 97 | 0 |  |
| 3 | 1.5 | 2.5 | run_command 0.30 | 976+5504 | 123 | 40 | 162 | 0 |  |
| 4 | 1.2 | 2.5 | run_command 4.55 | 2664+4096 | 264 | 148 | 602 | 0 |  |
| 5 | 8.3 | 9.4 | run_command 0.02 | 2293+6400 | 242 | 161 | 619 | 0 |  |
| 6 | 5.4 | 6.6 | run_command 0.14 | 465+8576 | 307 | 205 | 774 | 0 |  |
| 7 | 2.9 | 3.8 | run_command 0.48 | 491+8960 | 180 | 52 | 157 | 0 |  |
| 8 | 5.2 | 6.2 | set_plan 0.00, run_command 0.04 | 443+9344 | 237 | 85 | 286 | 0 |  |
| 9 | 3.8 | 4.2 | check_done 0.02 | 400+9728 | 53 | 0 | 0 | 0 |  |
| 10 | 4.5 | 5.5 |  | 377+10112 | 77 | 0 | 0 | 0 |  |

model total 46.7 s, tool total 5.6 s over 10 turns; gate: 8 actions exec 5.6 s, gate decision 0.00 s, 0 observations 0.0 s (0 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.3 | set_plan 0.00, list_windows 0.01 | 80+5376 | 136 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.1 | get_window_state 0.32 | 474+5376 | 107 | 7 | 31 | 0 |  |
| 3 | 2.9 | 9.7 | run_actions 2.57 | 4228+5760 | 1134 | 994 | 3848 | 0 |  |
| 4 | 1.7 | 3.9 | run_actions 1.29 | 2500+9984 | 407 | 276 | 1127 | 0 |  |
| 5 | 2.2 | 2.3 | get_window_state 0.22 | 818+12416 | 123 | 22 | 89 | 0 |  |
| 6 | 7.0 | 7.9 | run_actions 3.84 | 922+13184 | 243 | 82 | 274 | 1 | 139 KB image/jpeg 1396x823 |
| 7 | 19.3 | 20.8 | run_actions 1.40 | 1717+14080 | 282 | 152 | 614 | 2 | 144 KB image/jpeg 1396x823 |
| 8 | 16.1 | 19.3 | run_actions 0.59 | 3858+15744 | 458 | 361 | 1502 | 2 |  |
| 9 | 24.6 | 25.1 | run_actions 0.85 | 19465+4096 | 385 | 159 | 560 | 2 |  |
| 10 | 11.9 | 12.8 | run_actions 0.51 | 5838+19584 | 214 | 29 | 70 | 2 |  |
| 11 | 19.8 | 20.4 | get_window_state 0.19 | 2217+23552 | 101 | 0 | 0 | 2 |  |
| 12 | 29.9 | 36.9 | press_key 0.22 | 1328+25344 | 1015 | 936 | 3662 | 3 | 145 KB image/jpeg 1396x823 |
| 13 | 26.6 | 27.0 | get_window_state 0.31 | 1138+26624 | 101 | 0 | 0 | 3 |  |
| 14 | 25.1 | 27.5 | press_key 0.67 | 964+27648 | 409 | 311 | 1179 | 4 | 144 KB image/jpeg 1396x823 |
| 15 | 27.7 | 28.2 | get_window_state 0.19 | 552+28544 | 101 | 0 | 0 | 4 |  |
| 16 | 38.1 | 41.2 | press_key 0.10 | 943+29056 | 524 | 445 | 1892 | 5 | 151 KB image/jpeg 1396x823 |
| 17 | 43.0 | 43.8 | set_plan 0.00, press_key 0.84 | 646+29952 | 199 | 0 | 0 | 5 |  |
| 18 | 39.2 | 39.8 | list_windows 0.01 | 340+30592 | 56 | 11 | 54 | 5 |  |
| 19 | 39.7 | 41.0 | run_command 0.34 | 199+30848 | 200 | 0 | 0 | 5 |  |
| 20 | 45.5 | 47.1 | run_command 0.19 | 773+30976 | 302 | 147 | 529 | 5 |  |
| 21 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 459.1 s, tool total 14.7 s over 21 turns; gate: 20 actions exec 9.6 s, gate decision 0.14 s, 14 observations 4.0 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow01-libreoffice_calc-7efeb4b1-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.6 | set_plan 0.00, list_windows 0.01 | 80+5376 | 133 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.2 | get_window_state 0.46 | 467+5376 | 100 | 0 | 0 | 0 |  |
| 3 | 2.8 | 9.2 | run_command 0.11 | 4925+5760 | 880 | 754 | 2926 | 1 | 139 KB image/jpeg 1396x823 |
| 4 | 7.0 | 8.8 | run_command 0.12 | 2019+10624 | 302 | 156 | 601 | 1 |  |
| 5 | 10.9 | 14.8 | run_command 0.14 | 712+12544 | 699 | 518 | 2048 | 1 |  |
| 6 | 3.7 | 4.4 | run_actions 2.23 | 1914+13184 | 141 | 11 | 41 | 1 |  |
| 7 | 10.4 | 11.8 | check_done 0.12, set_plan 0.12 | 2908+14976 | 229 | 132 | 571 | 1 |  |
| 8 | 4.9 | 5.7 | run_actions 0.02 | 1611+17792 | 266 | 161 | 681 | 1 |  |
| 9 | 3.0 | 3.4 | press_key 0.89 | 453+19328 | 98 | 0 | 0 | 1 |  |
| 10 | 8.6 | 9.3 | get_window_state 0.21 | 343+19712 | 157 | 38 | 165 | 1 |  |
| 11 | 16.9 | 18.4 | press_key 0.60, run_command 0.72 | 1046+19968 | 305 | 107 | 464 | 2 | 143 KB image/jpeg 1396x823 |
| 12 | 3.3 | 5.0 | set_plan 0.00, check_done 0.14 | 16387+5376 | 144 | 50 | 180 | 2 |  |
| 13 | 3.7 | 4.1 |  | 2161+20992 | 73 | 0 | 0 | 2 |  |

model total 99.6 s, tool total 5.9 s over 13 turns; gate: 9 actions exec 1.9 s, gate decision 0.05 s, 6 observations 2.4 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow02-libreoffice_writer-0810415c-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 4.3 | set_plan 0.00 | 1286+4096 | 84 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.0 | list_windows 0.01 | 148+5376 | 28 | 0 | 0 | 0 |  |
| 3 | 1.5 | 2.0 | get_window_state 0.19 | 237+5504 | 92 | 9 | 38 | 0 |  |
| 4 | 4.3 | 6.4 | run_actions 4.55 | 927+5632 | 424 | 216 | 918 | 1 | 107 KB image/jpeg 1396x823 |
| 5 | 2.3 | 5.5 | press_key 0.85 | 3637+6528 | 526 | 447 | 2151 | 1 |  |
| 6 | 1.7 | 3.0 | run_actions 0.59 | 642+10112 | 241 | 78 | 331 | 1 |  |
| 7 | 2.0 | 2.5 | get_window_state 0.20 | 364+10752 | 83 | 0 | 0 | 1 |  |
| 8 | 2.4 | 3.0 | run_actions 2.43 | 940+11008 | 153 | 0 | 0 | 2 | 107 KB image/jpeg 1396x823 |
| 9 | 2.3 | 6.3 | run_actions 0.04 | 2255+11904 | 646 | 547 | 2392 | 2 |  |
| 10 | 3.2 | 5.9 | run_actions 2.11 | 850+14080 | 308 | 192 | 894 | 2 |  |
| 11 | 8.4 | 10.1 | run_actions 3.96 | 3472+14848 | 338 | 128 | 521 | 2 |  |
| 12 | 11.3 | 13.7 | run_actions 0.27 | 3503+18304 | 308 | 240 | 959 | 2 |  |
| 13 | 12.7 | 13.1 | get_window_state 0.18 | 463+21760 | 83 | 0 | 0 | 2 |  |
| 14 | 14.2 | 17.4 | run_actions 1.45 | 911+22144 | 504 | 385 | 1629 | 3 | 107 KB image/jpeg 1396x823 |
| 15 | 20.8 | 21.3 | click 0.93 | 2209+23040 | 121 | 27 | 103 | 4 | 107 KB image/jpeg 1396x823 |
| 16 | 16.0 | 16.7 | get_window_state 0.22 | 242+25216 | 83 | 0 | 0 | 4 |  |
| 17 | 25.3 | 26.5 | click 0.65 | 999+25344 | 160 | 66 | 280 | 5 | 107 KB image/jpeg 1396x823 |
| 18 | 25.5 | 25.9 | get_window_state 0.21 | 351+26240 | 83 | 0 | 0 | 5 |  |
| 19 | 27.0 | 27.9 | click 0.66 | 980+26496 | 168 | 74 | 236 | 6 | 105 KB image/jpeg 1396x823 |
| 20 | 27.8 | 28.5 | get_window_state 0.19 | 340+27392 | 83 | 0 | 0 | 6 |  |
| 21 | 35.0 | 35.8 | click 0.88 | 1022+27648 | 137 | 43 | 130 | 7 | 110 KB image/jpeg 1396x823 |
| 22 | 31.1 | 31.6 | get_window_state 0.20 | 351+28544 | 83 | 0 | 0 | 7 |  |
| 23 | 39.8 | 41.8 | click 1.54 | 927+28800 | 326 | 232 | 905 | 8 | 107 KB image/jpeg 1396x823 |
| 24 | 39.1 | 40.2 | click 0.84 | 445+29696 | 155 | 61 | 244 | 8 |  |
| 25 | 28.3 | 29.8 | click 0.67 | 304+30080 | 94 | 0 | 0 | 8 |  |
| 26 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 421.4 s, tool total 23.8 s over 26 turns; gate: 25 actions exec 18.4 s, gate decision 0.12 s, 14 observations 4.1 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow02-libreoffice_writer-0810415c-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.8 | set_plan 0.00, list_windows 0.03 | 6+5376 | 123 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.3 | get_window_state 0.29 | 375+5376 | 126 | 8 | 38 | 0 |  |
| 3 | 3.9 | 9.8 | run_actions 5.36 | 3499+5632 | 904 | 681 | 2988 | 1 | 107 KB image/jpeg 1396x823 |
| 4 | 8.0 | 10.4 | run_actions 0.58 | 2906+9088 | 452 | 298 | 1218 | 1 |  |
| 5 | 7.4 | 8.6 | click 0.72 | 2539+11904 | 186 | 138 | 565 | 1 |  |
| 6 | 8.0 | 9.5 | run_actions 0.56 | 3425+14336 | 273 | 136 | 637 | 1 |  |
| 7 | 7.6 | 8.6 | click 0.00 | 15076+4992 | 190 | 122 | 486 | 1 |  |
| 8 | 8.5 | 8.7 | click 4.53 | 2667+17664 | 48 | 0 | 0 | 1 |  |
| 9 | 3.0 | 3.7 | get_window_state 0.18 | 2389+20224 | 213 | 96 | 426 | 1 |  |
| 10 | 4.3 | 5.4 | press_key 0.32 | 791+22528 | 160 | 81 | 399 | 2 | 40 KB image/jpeg 920x607 |
| 11 | 6.5 | 6.7 | run_actions 1.23 | 251+23296 | 105 | 0 | 0 | 2 |  |
| 12 | 8.6 | 9.8 | run_actions 0.41 | 3291+23424 | 233 | 131 | 609 | 2 |  |
| 13 | 2.5 | 2.8 | get_window_state 0.00 | 448+26624 | 101 | 0 | 0 | 2 |  |
| 14 | 4.5 | 4.9 | get_window_state 0.24 | 252+27008 | 134 | 0 | 0 | 2 |  |
| 15 | 13.9 | 14.9 | list_windows 0.01, run_command 0.03 | 2260+27136 | 211 | 70 | 309 | 3 | 107 KB image/jpeg 1396x823 |
| 16 | 9.3 | 10.0 | run_command 0.06 | 421+29312 | 116 | 21 | 71 | 3 |  |
| 17 | 17.6 | 18.7 | run_command 0.04 | 281+29696 | 285 | 81 | 308 | 3 |  |
| 18 | 17.7 | 18.4 | run_command 0.04 | 367+29952 | 166 | 0 | 0 | 3 |  |
| 19 | 23.7 | 26.6 | run_command 0.04 | 1690+30208 | 249 | 77 | 340 | 3 |  |
| 20 | 15.6 | 16.7 | set_plan 0.00, check_done 0.09 | 512+31872 | 232 | 72 | 272 | 3 |  |
| 21 | 13.6 | 14.2 |  | 532+32384 | 56 | 0 | 0 | 3 |  |

model total 213.4 s, tool total 14.8 s over 21 turns; gate: 18 actions exec 7.6 s, gate decision 2.80 s, 13 observations 3.5 s (4 with screenshot), reviewer calls 1 (2.5 s)

## ow03-libreoffice_impress-39be0d19-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.3 | set_plan 0.00, list_windows 0.01 | 373+4992 | 124 | 0 | 0 | 0 |  |
| 2 | 1.6 | 2.1 | get_window_state 0.33 | 484+5248 | 101 | 19 | 64 | 0 |  |
| 3 | 2.2 | 2.6 | click 0.06 | 3968+5632 | 65 | 17 | 60 | 1 | 100 KB image/jpeg 1396x823 |
| 4 | 1.9 | 2.4 | get_window_state 0.21 | 137+9600 | 101 | 0 | 0 | 1 |  |
| 5 | 10.4 | 11.1 | click 1.97 | 859+9728 | 125 | 31 | 111 | 2 | 100 KB image/jpeg 1396x823 |
| 6 | 10.7 | 11.1 | get_window_state 0.20 | 306+10496 | 83 | 0 | 0 | 2 |  |
| 7 | 5.3 | 6.7 | click 0.02 | 882+10752 | 153 | 105 | 445 | 3 | 89 KB image/jpeg 1396x823 |
| 8 | 13.2 | 13.7 | click 0.80 | 368+11520 | 94 | 0 | 0 | 3 |  |
| 9 | 6.7 | 6.8 | get_window_state 0.21 | 296+11776 | 83 | 0 | 0 | 3 |  |
| 10 | 3.5 | 3.9 | click 1.10 | 925+12032 | 106 | 0 | 0 | 4 | 91 KB image/jpeg 1396x823 |
| 11 | 16.6 | 16.6 | get_window_state 0.19 | 225+12928 | 83 | 0 | 0 | 4 |  |
| 12 | 20.3 | 21.2 | press_key 0.11 | 982+13056 | 132 | 40 | 156 | 5 | 98 KB image/jpeg 1396x823 |
| 13 | 22.9 | 23.3 | press_key 0.90 | 283+13952 | 79 | 0 | 0 | 5 |  |
| 14 | 11.4 | 11.6 | press_key 0.11 | 171+14208 | 98 | 0 | 0 | 5 |  |
| 15 | 17.1 | 17.5 | get_window_state 0.21 | 206+14336 | 83 | 0 | 0 | 5 |  |
| 16 | 8.4 | 9.2 | press_key 0.11 | 963+14464 | 146 | 47 | 200 | 6 | 89 KB image/jpeg 1396x823 |
| 17 | 27.2 | 27.6 | click 0.66 | 278+15360 | 94 | 0 | 0 | 6 |  |
| 18 | 27.2 | 27.7 | get_window_state 0.17 | 206+15616 | 83 | 0 | 0 | 6 |  |
| 19 | 24.3 | 25.2 | get_window_state 0.19 | 963+15744 | 118 | 0 | 0 | 7 | 91 KB image/jpeg 1396x823 |
| 20 | 34.7 | 35.4 | get_window_state 0.66 | 504+16640 | 136 | 20 | 97 | 8 | 16 KB image/jpeg 279x467 |
| 21 | 41.0 | 41.3 | click 0.02 | 709+17024 | 48 | 0 | 0 | 9 | 16 KB image/jpeg 279x467 |
| 22 | 35.0 | 36.7 | click 1.42 | 219+17664 | 314 | 220 | 753 | 9 |  |
| 23 | 35.8 | 38.0 | click 0.60 | 548+17792 | 385 | 291 | 975 | 9 |  |
| 24 | 32.9 | 33.0 | get_window_state 0.20 | 511+18304 | 83 | 0 | 0 | 9 |  |
| 25 | 35.4 | 36.2 | get_window_state 0.53 | 620+18688 | 151 | 69 | 266 | 10 | 59 KB image/jpeg 979x499 |
| 26 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 463.1 s, tool total 11.0 s over 26 turns; gate: 13 actions exec 7.7 s, gate decision 0.10 s, 12 observations 2.0 s (11 with screenshot), reviewer calls 0 (0.0 s)

## ow03-libreoffice_impress-39be0d19-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 4.0 | set_plan 0.00, list_windows 0.01 | 117+5248 | 139 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.1 | get_window_state 0.28 | 517+5248 | 91 | 9 | 35 | 0 |  |
| 3 | 7.7 | 8.5 | click 0.39 | 3863+5760 | 134 | 86 | 345 | 1 | 100 KB image/jpeg 1396x823 |
| 4 | 4.8 | 5.4 | get_window_state 0.00 | 1186+9600 | 111 | 28 | 102 | 1 |  |
| 5 | 2.7 | 3.9 | get_window_state 0.21 | 230+10752 | 153 | 54 | 282 | 1 |  |
| 6 | 4.7 | 5.6 | click 2.30 | 1004+10880 | 186 | 92 | 386 | 2 | 100 KB image/jpeg 1396x823 |
| 7 | 8.8 | 9.3 | click 1.32 | 3498+11776 | 78 | 30 | 112 | 3 | 89 KB image/jpeg 1396x823 |
| 8 | 16.5 | 16.8 | click 1.79 | 3130+15232 | 48 | 0 | 0 | 4 | 91 KB image/jpeg 1396x823 |
| 9 | 6.9 | 9.0 | run_actions 7.12 | 429+18304 | 410 | 155 | 569 | 4 |  |
| 10 | 28.1 | 30.4 | run_actions 3.06 | 970+18688 | 401 | 246 | 945 | 4 |  |
| 11 | 23.0 | 24.0 | press_key 1.22 | 789+19584 | 160 | 100 | 297 | 4 |  |
| 12 | 4.6 | 4.8 | click 1.01 | 388+20352 | 105 | 21 | 62 | 4 |  |
| 13 | 4.9 | 5.4 | get_window_state 0.19 | 205+20736 | 99 | 0 | 0 | 4 |  |
| 14 | 25.6 | 26.1 | press_key 1.38 | 925+20864 | 118 | 58 | 250 | 5 | 106 KB image/jpeg 1396x823 |
| 15 | 23.2 | 23.6 | press_key 1.39 | 2437+21760 | 95 | 16 | 63 | 5 |  |
| 16 | 20.4 | 21.0 | list_windows 0.01 | 2393+24192 | 95 | 49 | 205 | 5 |  |
| 17 | 21.9 | 23.0 | run_command 0.03 | 239+26496 | 184 | 76 | 356 | 5 |  |
| 18 | 28.2 | 29.2 | get_window_state 0.10 | 403+26624 | 199 | 100 | 424 | 5 |  |
| 19 | 35.0 | 38.0 | run_command 0.01 | 569+27008 | 554 | 376 | 1572 | 6 | 60 KB image/jpeg 979x499 |
| 20 | 3.4 | 4.2 | run_command 0.04 | 726+27520 | 189 | 0 | 0 | 6 |  |
| 21 | 16.3 | 17.8 | set_plan 0.00 | 396+28160 | 230 | 175 | 586 | 6 |  |
| 22 | 13.2 | 13.4 | check_done 0.09 | 323+28544 | 55 | 0 | 0 | 6 |  |
| 23 | 18.0 | 19.1 |  | 365+28800 | 62 | 0 | 0 | 6 |  |

model total 344.5 s, tool total 22.0 s over 23 turns; gate: 20 actions exec 15.7 s, gate decision 0.69 s, 16 observations 4.6 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow04-chrome-af630914-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.3 | set_plan 0.00 | 1235+4096 | 102 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.1 | list_windows 0.01 | 246+5248 | 28 | 0 | 0 | 0 |  |
| 3 | 1.2 | 1.8 | get_window_state 0.11 | 334+5376 | 127 | 44 | 170 | 0 |  |
| 4 | 2.0 | 2.7 | run_actions 2.60 | 777+5632 | 141 | 0 | 0 | 1 | 50 KB image/jpeg 1050x853 |
| 5 | 3.9 | 5.4 | get_window_state 0.12 | 2920+6400 | 259 | 159 | 669 | 1 |  |
| 6 | 6.9 | 9.6 | run_actions 3.07 | 958+9216 | 453 | 335 | 1166 | 2 | 69 KB image/jpeg 1050x853 |
| 7 | 8.0 | 9.0 | run_command 0.07 | 2976+10112 | 206 | 77 | 361 | 3 | 77 KB image/jpeg 1050x853 |
| 8 | 13.3 | 14.6 | run_command 0.02 | 352+13056 | 252 | 156 | 670 | 3 |  |
| 9 | 16.6 | 17.2 | set_plan 0.00 | 571+13312 | 108 | 53 | 245 | 3 |  |
| 10 | 14.8 | 15.0 | check_done 0.13 | 232+13824 | 28 | 0 | 0 | 3 |  |
| 11 | 12.6 | 13.3 |  | 267+13952 | 69 | 0 | 0 | 3 |  |

model total 92.9 s, tool total 6.1 s over 11 turns; gate: 7 actions exec 4.5 s, gate decision 0.03 s, 5 observations 1.1 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow04-chrome-af630914-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.5 | 4.2 | set_plan 0.00, list_windows 0.01 | 83+5248 | 132 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.0 | get_window_state 0.37 | 471+5248 | 84 | 20 | 69 | 0 |  |
| 3 | 3.7 | 4.5 | run_actions 3.89 | 4012+5632 | 151 | 0 | 0 | 1 | 50 KB image/jpeg 1050x853 |
| 4 | 5.5 | 6.8 | set_plan 0.00, run_actions 3.17 | 2968+9600 | 298 | 144 | 596 | 1 |  |
| 5 | 5.6 | 8.4 | run_actions 1.41, run_command 3.46 | 2297+12544 | 453 | 145 | 592 | 1 |  |
| 6 | 5.5 | 6.0 | get_window_state 0.29 | 1093+14720 | 119 | 38 | 192 | 1 |  |
| 7 | 4.9 | 6.5 | run_command 0.02 | 789+15744 | 304 | 143 | 574 | 1 |  |
| 8 | 6.7 | 9.5 | click 1.40, run_command 1.44 | 395+16512 | 433 | 237 | 1050 | 1 |  |
| 9 | 4.7 | 6.8 | run_command 20.69, press_key 21.99 | 1302+16896 | 414 | 179 | 710 | 1 |  |
| 10 | 5.2 | 6.1 | run_command 0.02 | 1193+18176 | 166 | 33 | 148 | 1 |  |
| 11 | 4.7 | 5.9 | run_actions 1.31 | 296+19328 | 227 | 129 | 498 | 1 |  |
| 12 | 6.0 | 6.7 | get_window_state 0.38, run_command 8.41 | 651+19584 | 180 | 0 | 0 | 1 |  |
| 13 | 8.9 | 10.1 | set_plan 0.00, click 1.49 | 869+20224 | 237 | 111 | 469 | 1 |  |
| 14 | 6.2 | 7.0 | get_window_state 0.93 | 823+20992 | 149 | 31 | 149 | 1 |  |
| 15 | 8.8 | 9.3 | check_done 0.03 | 1434+21760 | 94 | 37 | 151 | 2 | 78 KB image/jpeg 1050x853 |
| 16 | 15.7 | 15.9 |  | 275+23168 | 70 | 0 | 0 | 2 |  |

model total 115.5 s, tool total 70.7 s over 16 turns; gate: 16 actions exec 40.4 s, gate decision 0.07 s, 12 observations 6.0 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow05-libreoffice_calc-abed40dc-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.5 | set_plan 0.00, list_apps 0.37 | 1372+4096 | 137 | 20 | 97 | 0 |  |
| 2 | 2.3 | 2.5 | list_windows 0.01 | 1847+5376 | 44 | 0 | 0 | 0 |  |
| 3 | 6.5 | 6.5 | get_window_state 0.52 | 157+7168 | 64 | 0 | 0 | 0 |  |
| 4 | 10.1 | 10.5 | run_command 0.05 | 4323+7296 | 301 | 219 | 851 | 1 | 122 KB image/jpeg 1396x823 |
| 5 | 4.2 | 4.5 | run_command 0.05 | 538+11520 | 128 | 0 | 0 | 1 |  |
| 6 | 6.9 | 10.2 | run_actions 14.65 | 9236+4096 | 695 | 322 | 1247 | 1 |  |
| 7 | 3.0 | 6.9 | run_actions 6.60 | 3966+12032 | 741 | 492 | 1849 | 1 |  |
| 8 | 3.2 | 5.8 | run_actions 0.75 | 1754+15872 | 520 | 213 | 789 | 1 |  |
| 9 | 3.6 | 3.9 | get_window_state 0.23 | 734+17536 | 83 | 0 | 0 | 1 |  |
| 10 | 6.6 | 7.7 | run_actions 13.43 | 926+18176 | 317 | 10 | 26 | 2 | 123 KB image/jpeg 1396x823 |
| 11 | 7.5 | 8.8 | run_actions 2.26 | 1196+19072 | 233 | 128 | 549 | 2 |  |
| 12 | 16.4 | 17.4 | run_command 0.05 | 476+20224 | 206 | 25 | 120 | 2 |  |
| 13 | 13.5 | 14.3 | set_plan 0.00, check_done 0.20 | 830+20608 | 135 | 33 | 131 | 2 |  |
| 14 | 17.5 | 18.6 |  | 774+21376 | 66 | 0 | 0 | 2 |  |

model total 120.1 s, tool total 39.2 s over 14 turns; gate: 41 actions exec 33.6 s, gate decision 0.22 s, 8 observations 4.0 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow05-libreoffice_calc-abed40dc-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.1 | 2.8 | set_plan 0.01, list_apps 0.03 | 92+5376 | 128 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.1 | list_windows 0.02 | 534+5376 | 58 | 14 | 53 | 0 |  |
| 3 | 2.3 | 2.6 | get_window_state 0.25 | 138+5888 | 100 | 0 | 0 | 0 |  |
| 4 | 1.8 | 4.5 | run_actions 2.96 | 2637+6016 | 506 | 368 | 1440 | 0 |  |
| 5 | 1.6 | 2.5 | press_key 2.00 | 2033+8576 | 170 | 72 | 287 | 0 |  |
| 6 | 1.6 | 4.2 | run_actions 15.53 | 565+10496 | 494 | 144 | 538 | 0 |  |
| 7 | 5.0 | 5.7 | press_key 1.39 | 1491+11008 | 125 | 27 | 116 | 0 |  |
| 8 | 1.7 | 2.4 | run_command 0.02, list_windows 0.03 | 381+12416 | 143 | 42 | 210 | 0 |  |
| 9 | 5.7 | 6.4 | run_command 0.29 | 442+12672 | 132 | 21 | 72 | 0 |  |
| 10 | 4.1 | 4.7 | check_done 0.16, set_plan 0.16 | 555+13056 | 102 | 0 | 0 | 0 |  |
| 11 | 4.1 | 5.0 |  | 740+13568 | 66 | 0 | 0 | 0 |  |

model total 42.8 s, tool total 22.8 s over 11 turns; gate: 23 actions exec 18.8 s, gate decision 0.16 s, 7 observations 3.1 s (0 with screenshot), reviewer calls 0 (0.0 s)

## ow06-chrome-3720f614-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.5 | list_apps 0.02 | 82+5248 | 144 | 116 | 517 | 0 |  |
| 2 | 2.0 | 2.1 | list_windows 0.01 | 1746+5248 | 42 | 14 | 57 | 0 |  |
| 3 | 1.2 | 3.0 | run_command 0.03 | 2286+4864 | 231 | 142 | 637 | 0 |  |
| 4 | 3.0 | 4.5 | run_command 0.03 | 529+6912 | 280 | 134 | 598 | 0 |  |
| 5 | 6.9 | 7.0 | run_command 0.02 | 344+7424 | 87 | 18 | 73 | 0 |  |
| 6 | 1.9 | 2.3 | run_command 0.41 | 271+7680 | 105 | 0 | 0 | 0 |  |
| 7 | 4.8 | 5.7 | set_plan 0.00 | 180+7936 | 215 | 78 | 344 | 0 |  |
| 8 | 3.5 | 4.4 | run_command 8.01 | 347+8064 | 169 | 93 | 453 | 0 |  |
| 9 | 3.4 | 3.7 | list_windows 0.02 | 314+8320 | 28 | 0 | 0 | 0 |  |
| 10 | 1.6 | 3.4 | run_actions 4.96 | 274+8576 | 170 | 27 | 108 | 0 |  |
| 11 | 3.1 | 3.6 | click 1.50 | 6717+4992 | 92 | 44 | 172 | 0 |  |
| 12 | 4.4 | 4.5 | get_window_state 1.43 | 3052+8832 | 79 | 0 | 0 | 0 |  |
| 13 | 12.1 | 12.1 | type_text 3.06 | 1277+11776 | 85 | 17 | 72 | 1 | 63 KB image/jpeg 1050x853 |
| 14 | 5.3 | 6.1 | get_window_state 0.43 | 271+12928 | 82 | 0 | 0 | 1 |  |
| 15 | 11.5 | 12.8 | run_actions 2.95 | 699+13184 | 241 | 135 | 610 | 2 | 57 KB image/jpeg 1050x853 |
| 16 | 8.4 | 8.7 | check_done 0.03 | 1537+13824 | 28 | 0 | 0 | 2 |  |
| 17 | 4.1 | 4.9 |  | 196+15360 | 101 | 0 | 0 | 2 |  |

model total 91.4 s, tool total 22.9 s over 17 turns; gate: 11 actions exec 16.9 s, gate decision 0.04 s, 6 observations 4.7 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow06-chrome-3720f614-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.8 | 4.1 | set_plan 0.00, list_windows 0.22 | 210+5120 | 172 | 79 | 381 | 0 |  |
| 2 | 1.9 | 2.1 | run_command 0.13 | 420+5248 | 87 | 15 | 60 | 0 |  |
| 3 | 2.9 | 3.6 | list_apps 0.00 | 197+5632 | 100 | 56 | 281 | 0 |  |
| 4 | 3.6 | 4.0 | list_apps 0.77 | 224+5760 | 28 | 0 | 0 | 0 |  |
| 5 | 4.0 | 5.8 | run_command 0.02 | 357+5888 | 207 | 125 | 625 | 0 |  |
| 6 | 2.8 | 3.6 | run_command 8.18 | 440+6144 | 88 | 0 | 0 | 0 |  |
| 7 | 3.5 | 3.7 | list_windows 0.01 | 206+6528 | 28 | 0 | 0 | 0 |  |
| 8 | 2.4 | 2.9 | get_window_state 0.30 | 294+6656 | 128 | 46 | 199 | 0 |  |
| 9 | 2.0 | 2.6 | run_actions 2.50 | 2611+6912 | 133 | 8 | 40 | 1 | 50 KB image/jpeg 1050x853 |
| 10 | 2.6 | 3.0 | click 2.02 | 2863+9472 | 95 | 30 | 111 | 1 |  |
| 11 | 6.4 | 7.1 | run_actions 2.78 | 3401+12288 | 139 | 18 | 76 | 1 |  |
| 12 | 2.9 | 3.5 | get_window_state 0.34 | 1191+15616 | 128 | 31 | 145 | 1 |  |
| 13 | 2.2 | 4.5 | click 1.20 | 1263+16768 | 290 | 225 | 1077 | 2 | 57 KB image/jpeg 1050x853 |
| 14 | 10.5 | 11.2 | set_plan 0.00 | 544+17920 | 203 | 0 | 0 | 2 |  |
| 15 | 5.1 | 5.2 | check_done 0.02 | 359+18432 | 28 | 0 | 0 | 2 |  |
| 16 | 11.3 | 12.3 |  | 270+18688 | 85 | 0 | 0 | 2 |  |

model total 79.1 s, tool total 18.5 s over 16 turns; gate: 9 actions exec 15.1 s, gate decision 0.05 s, 8 observations 2.3 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow07-libreoffice_writer-ecc2413d-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.4 | 2.5 | set_plan 0.00, list_windows 0.01 | 1287+4096 | 118 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.4 | get_window_state 0.19 | 368+5376 | 135 | 52 | 224 | 0 |  |
| 3 | 8.8 | 21.0 | scroll 1.28 | 973+5632 | 2061 | 1935 | 8218 | 1 | 82 KB image/jpeg 1396x823 |
| 4 | 7.8 | 8.1 | get_window_state 0.19 | 2183+6528 | 83 | 0 | 0 | 1 |  |
| 5 | 5.1 | 13.6 | run_command 0.03 | 828+8704 | 1330 | 1054 | 4563 | 2 | 71 KB image/jpeg 1396x823 |
| 6 | 14.1 | 14.6 | run_command 0.02 | 1491+9472 | 100 | 14 | 48 | 2 |  |
| 7 | 8.5 | 9.1 | run_command 0.02 | 739+10880 | 87 | 0 | 0 | 2 |  |
| 8 | 15.6 | 16.8 | run_command 0.05 | 442+11520 | 254 | 0 | 0 | 2 |  |
| 9 | 12.8 | 21.5 | scroll 1.80 | 1553+11904 | 1448 | 1322 | 5257 | 2 |  |
| 10 | 6.9 | 7.0 | get_window_state 0.20 | 1510+13440 | 123 | 40 | 177 | 2 |  |
| 11 | 7.1 | 8.4 | click 1.44 | 963+14848 | 229 | 135 | 514 | 3 | 82 KB image/jpeg 1396x823 |
| 12 | 8.1 | 8.6 | get_window_state 0.22 | 384+15744 | 99 | 0 | 0 | 3 |  |
| 13 | 9.0 | 21.7 | run_actions 6.71 | 837+16128 | 2007 | 1782 | 6605 | 4 | 82 KB image/jpeg 1396x823 |
| 14 | 24.1 | 25.2 | press_key 0.91 | 4942+16896 | 239 | 141 | 526 | 4 |  |
| 15 | 19.6 | 20.1 | get_window_state 0.21 | 380+21760 | 83 | 0 | 0 | 4 |  |
| 16 | 10.7 | 13.0 | press_key 0.88 | 956+22016 | 420 | 322 | 1140 | 5 | 71 KB image/jpeg 1396x823 |
| 17 | 11.2 | 11.7 | get_window_state 0.18 | 543+22912 | 83 | 0 | 0 | 5 |  |
| 18 | 12.1 | 13.3 | list_windows 0.01, run_command 0.03 | 863+23424 | 202 | 99 | 449 | 6 | 71 KB image/jpeg 1396x823 |
| 19 | 26.7 | 28.9 | run_command 0.04 | 469+24192 | 370 | 130 | 533 | 6 |  |
| 20 | 5.7 | 6.0 | set_plan 0.00, check_done 0.10 | 691+24576 | 295 | 192 | 666 | 6 |  |
| 21 | 30.3 | 30.7 |  | 658+25216 | 54 | 0 | 0 | 6 |  |

model total 304.2 s, tool total 14.5 s over 21 turns; gate: 17 actions exec 11.6 s, gate decision 0.13 s, 9 observations 1.9 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow07-libreoffice_writer-ecc2413d-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 4.0 | 4.2 | list_windows 0.01, set_plan 0.01 | 7+5376 | 120 | 0 | 0 | 0 |  |
| 2 | 1.6 | 1.8 | get_window_state 0.31 | 366+5376 | 138 | 38 | 157 | 0 |  |
| 3 | 2.1 | 4.0 | get_window_state 0.23 | 3590+5632 | 306 | 223 | 972 | 0 |  |
| 4 | 6.2 | 14.2 | run_actions 0.96 | 1061+9216 | 1416 | 1280 | 5407 | 1 | 82 KB image/jpeg 1396x823 |
| 5 | 2.4 | 2.8 | scroll 3.10 | 7841+4096 | 135 | 9 | 30 | 1 |  |
| 6 | 25.8 | 26.6 | run_actions 2.89 | 3478+10240 | 273 | 138 | 560 | 3 | 83 KB image/jpeg 1396x823; 83 KB image/jpeg 1396x823 |
| 7 | 18.8 | 19.9 | scroll 2.75 | 1309+13696 | 146 | 20 | 80 | 4 | 83 KB image/jpeg 1396x823 |
| 8 | 2.9 | 4.1 | scroll 2.15 | 3511+14976 | 225 | 99 | 396 | 5 | 66 KB image/jpeg 1396x823 |
| 9 | 7.3 | 12.6 | run_command 0.03 | 3720+18432 | 1024 | 943 | 3788 | 6 | 82 KB image/jpeg 1396x823 |
| 10 | 5.1 | 6.0 | run_command 0.05 | 1261+22144 | 163 | 19 | 95 | 6 |  |
| 11 | 3.0 | 10.1 | scroll 1.98 | 2059+23296 | 1224 | 1098 | 4604 | 6 |  |
| 12 | 2.6 | 12.9 | run_actions 6.83 | 4654+25344 | 1516 | 1267 | 4897 | 7 | 69 KB image/jpeg 1396x823 |
| 13 | 24.5 | 24.9 | run_command 0.02 | 4482+29952 | 66 | 22 | 105 | 7 |  |
| 14 | 24.6 | 24.9 | get_window_state 0.19 | 126+34432 | 83 | 0 | 0 | 7 |  |
| 15 | 29.7 | 41.2 | press_key 1.31 | 958+34432 | 421 | 323 | 1257 | 8 | 72 KB image/jpeg 1396x823 |
| 16 | 36.1 | 36.9 | get_window_state 0.28 | 3271+35328 | 165 | 82 | 317 | 8 |  |
| 17 | 18.4 | 18.5 | press_key 1.30 | 987+38528 | 204 | 106 | 412 | 9 | 83 KB image/jpeg 1396x823 |
| 18 | 46.6 | 47.2 | run_command 0.02, list_windows 0.03 | 1056+39424 | 131 | 53 | 247 | 9 |  |
| 19 | 30.9 | 33.9 | run_command 0.11 | 456+40448 | 442 | 292 | 1128 | 9 |  |
| 20 | 16.8 | 21.6 | run_command 0.06 | 1565+40832 | 743 | 533 | 2128 | 9 |  |
| 21 | 51.8 | 54.2 | run_command 0.04 | 1160+42368 | 378 | 121 | 463 | 9 |  |
| 22 | 16.0 | 20.9 | run_actions 2.85 | 449+43520 | 517 | 396 | 1609 | 9 |  |
| 23 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 443.4 s, tool total 27.5 s over 23 turns; gate: 24 actions exec 17.9 s, gate decision 1.68 s, 17 observations 6.7 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow08-libreoffice_calc-1e8df695-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.6 | 3.7 | set_plan 0.10, list_apps 0.10 | 1364+4096 | 134 | 0 | 0 | 0 |  |
| 2 | 2.4 | 2.7 | list_windows 0.01 | 1831+5376 | 46 | 0 | 0 | 0 |  |
| 3 | 7.7 | 8.9 | get_window_state 0.61 | 139+7168 | 82 | 0 | 0 | 0 |  |
| 4 | 11.6 | 14.6 | run_actions 7.32 | 2946+7296 | 580 | 348 | 1225 | 1 | 130 KB image/jpeg 1396x823 |
| 5 | 4.5 | 6.3 | run_actions 3.49 | 3870+10240 | 280 | 115 | 385 | 1 |  |
| 6 | 5.8 | 7.2 | run_actions 1.08 | 1030+14080 | 244 | 106 | 421 | 1 |  |
| 7 | 11.1 | 11.3 | get_window_state 0.75 | 11384+4096 | 99 | 0 | 0 | 1 |  |
| 8 | 23.6 | 24.9 | run_actions 5.63 | 1310+15104 | 265 | 95 | 311 | 2 | 126 KB image/jpeg 1396x823 |
| 9 | 6.2 | 10.6 | run_actions 4.23 | 3240+16384 | 745 | 578 | 1982 | 2 |  |
| 10 | 30.3 | 30.4 | run_actions 1.57 | 1717+19584 | 171 | 66 | 203 | 2 |  |
| 11 | 13.2 | 14.5 | run_command 0.29, list_windows 0.29 | 424+21248 | 231 | 74 | 355 | 2 |  |
| 12 | 14.0 | 14.3 | set_plan 0.00, check_done 0.20 | 696+21632 | 116 | 18 | 79 | 2 |  |
| 13 | 22.0 | 22.3 |  | 990+22272 | 56 | 0 | 0 | 2 |  |

model total 172.0 s, tool total 25.7 s over 13 turns; gate: 23 actions exec 19.1 s, gate decision 0.11 s, 10 observations 5.3 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow08-libreoffice_calc-1e8df695-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.5 | 3.0 | set_plan 0.00, list_windows 0.01 | 84+5376 | 130 | 0 | 0 | 0 |  |
| 2 | 2.4 | 2.8 | get_window_state 0.70 | 465+5376 | 100 | 0 | 0 | 0 |  |
| 3 | 6.7 | 7.3 | press_key 0.73 | 2817+5760 | 244 | 146 | 531 | 0 |  |
| 4 | 1.7 | 7.4 | get_window_state 0.42 | 1418+8576 | 596 | 496 | 1859 | 0 |  |
| 5 | 8.2 | 9.1 | run_actions 2.28 | 2984+9984 | 293 | 73 | 201 | 1 | 132 KB image/jpeg 1396x823 |
| 6 | 21.3 | 21.9 | run_actions 4.14 | 3912+12928 | 199 | 17 | 74 | 2 | 132 KB image/jpeg 1396x823 |
| 7 | 8.4 | 9.2 | run_actions 0.42 | 2146+16768 | 183 | 22 | 53 | 2 |  |
| 8 | 15.0 | 15.7 | get_window_state 0.00 | 13846+5376 | 101 | 0 | 0 | 2 |  |
| 9 | 12.2 | 12.7 | get_window_state 0.09 | 592+18816 | 117 | 0 | 0 | 2 |  |
| 10 | 18.8 | 21.0 | run_actions 2.27 | 599+19328 | 333 | 166 | 566 | 3 | 72 KB image/jpeg 979x499 |
| 11 | 15.2 | 15.8 | press_key 0.85 | 1951+19200 | 128 | 30 | 117 | 3 |  |
| 12 | 20.8 | 21.5 | get_window_state 0.15 | 1613+19840 | 140 | 23 | 117 | 3 |  |
| 13 | 22.4 | 22.5 | run_command 0.03 | 619+21376 | 101 | 39 | 178 | 4 | 77 KB image/jpeg 979x499 |
| 14 | 22.4 | 23.5 | run_command 0.06 | 326+21888 | 258 | 34 | 134 | 4 |  |
| 15 | 21.0 | 22.6 | run_command 0.04 | 1011+22144 | 347 | 113 | 450 | 4 |  |
| 16 | 24.1 | 24.5 | set_plan 0.00 | 696+23040 | 55 | 0 | 0 | 4 |  |
| 17 | 22.4 | 22.7 | check_done 0.12 | 184+23680 | 53 | 0 | 0 | 4 |  |
| 18 | 23.6 | 24.1 |  | 815+23808 | 61 | 0 | 0 | 4 |  |

model total 287.2 s, tool total 12.3 s over 18 turns; gate: 17 actions exec 7.4 s, gate decision 0.08 s, 12 observations 4.2 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow09-libreoffice_impress-b8adbc24-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.6 | set_plan 0.00, list_windows 0.01 | 382+4992 | 140 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.2 | get_window_state 0.39 | 528+5248 | 74 | 10 | 39 | 0 |  |
| 3 | 8.4 | 9.9 | click 0.05 | 5156+5760 | 372 | 307 | 1250 | 1 | 110 KB image/jpeg 1396x823 |
| 4 | 5.3 | 5.8 | get_window_state 0.44 | 486+10880 | 81 | 0 | 0 | 1 |  |
| 5 | 8.4 | 9.5 | click 1.93 | 1155+11264 | 222 | 128 | 517 | 2 | 110 KB image/jpeg 1396x823 |
| 6 | 10.3 | 10.6 | get_window_state 0.20 | 321+12416 | 83 | 0 | 0 | 2 |  |
| 7 | 13.4 | 15.5 | run_command 0.05 | 897+12672 | 366 | 120 | 487 | 3 | 92 KB image/jpeg 1396x823 |
| 8 | 14.3 | 14.7 | run_command 7.23 | 505+13568 | 89 | 16 | 52 | 3 |  |
| 9 | 6.5 | 6.9 | run_command 0.04 | 328+13952 | 114 | 0 | 0 | 3 |  |
| 10 | 15.0 | 15.9 | run_command 0.02 | 2119+14208 | 154 | 56 | 148 | 3 |  |
| 11 | 17.6 | 20.4 | run_actions 4.66 | 1371+16256 | 566 | 428 | 1620 | 3 |  |
| 12 | 34.8 | 35.1 | run_actions 2.61 | 3824+17536 | 285 | 159 | 617 | 4 | 87 KB image/jpeg 1396x823 |
| 13 | 21.2 | 22.6 | click 0.98 | 2214+21248 | 226 | 142 | 555 | 4 |  |
| 14 | 23.6 | 24.1 | get_window_state 0.21 | 362+23424 | 123 | 40 | 185 | 4 |  |
| 15 | 30.3 | 33.5 | click 0.99 | 978+23680 | 527 | 433 | 1781 | 5 | 86 KB image/jpeg 1396x823 |
| 16 | 27.3 | 27.7 | get_window_state 0.10 | 705+24576 | 83 | 0 | 0 | 5 |  |
| 17 | 26.5 | 27.8 | click 0.91 | 603+25216 | 230 | 136 | 525 | 6 | 55 KB image/jpeg 979x499 |
| 18 | 31.9 | 33.0 | get_window_state 0.10 | 417+25728 | 83 | 0 | 0 | 6 |  |
| 19 | 32.8 | 34.3 | get_window_state 0.38 | 525+26112 | 211 | 147 | 597 | 7 | 51 KB image/jpeg 979x499 |
| 20 | 46.1 | 47.8 | run_actions 6.32 | 5502+26624 | 365 | 144 | 501 | 8 | 35 KB image/jpeg 622x551 |
| 21 | 42.3 | 43.0 | get_window_state 0.09 | 1065+32000 | 177 | 94 | 306 | 8 |  |
| 22 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 443.0 s, tool total 27.7 s over 22 turns; gate: 21 actions exec 23.2 s, gate decision 0.08 s, 12 observations 3.4 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow09-libreoffice_impress-b8adbc24-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.6 | 2.6 | set_plan 0.00, list_windows 0.01 | 126+5248 | 141 | 0 | 0 | 0 |  |
| 2 | 17.3 | 17.8 | run_command 0.03, get_window_state 0.34 | 527+5248 | 233 | 110 | 472 | 0 |  |
| 3 | 4.9 | 6.4 | run_command 0.04 | 5000+5760 | 334 | 51 | 194 | 1 | 62 KB image/jpeg 979x499 |
| 4 | 3.0 | 3.5 | run_command 0.21 | 460+10752 | 99 | 22 | 71 | 1 |  |
| 5 | 4.8 | 7.5 | click 0.46 | 1039+11136 | 385 | 320 | 1374 | 1 |  |
| 6 | 3.2 | 3.9 | get_window_state 0.07 | 535+12160 | 126 | 63 | 252 | 1 |  |
| 7 | 3.2 | 4.3 | get_window_state 0.11 | 1450+12672 | 149 | 86 | 371 | 1 |  |
| 8 | 8.8 | 10.4 | run_actions 1.55 | 755+14080 | 274 | 151 | 587 | 2 | 62 KB image/jpeg 979x499 |
| 9 | 14.2 | 17.3 | get_window_state 0.39 | 3297+14720 | 510 | 427 | 1756 | 3 | 62 KB image/jpeg 979x499 |
| 10 | 27.0 | 31.6 | press_key 1.34 | 1299+17920 | 734 | 653 | 2518 | 4 | 62 KB image/jpeg 979x499 |
| 11 | 14.8 | 16.4 | run_command 0.04 | 3546+19200 | 328 | 190 | 833 | 5 | 57 KB image/jpeg 979x499 |
| 12 | 12.8 | 13.8 | run_command 0.04 | 1425+22656 | 161 | 50 | 195 | 5 |  |
| 13 | 25.6 | 31.2 | press_key 1.88 | 1238+24064 | 665 | 586 | 2378 | 5 |  |
| 14 | 34.6 | 34.9 | run_actions 4.29 | 3393+25216 | 393 | 231 | 811 | 6 | 53 KB image/jpeg 979x499 |
| 15 | 28.9 | 29.6 | click 1.34 | 2954+28544 | 119 | 53 | 215 | 6 |  |
| 16 | 7.2 | 7.6 | click 2.21 | 2127+31488 | 66 | 0 | 0 | 6 |  |
| 17 | 31.8 | 33.7 | get_window_state 0.12 | 3277+33536 | 332 | 250 | 1009 | 6 |  |
| 18 | 41.0 | 45.4 | run_actions 2.39 | 34313+4096 | 773 | 635 | 2448 | 7 | 35 KB image/jpeg 622x551 |
| 19 | 36.1 | 36.6 | get_window_state 0.01 | 2624+36736 | 82 | 0 | 0 | 7 |  |
| 20 | 21.3 | 22.0 | list_windows 0.01 | 182+39296 | 101 | 56 | 254 | 7 |  |
| 21 | 27.0 | 30.0 | get_window_state 0.47 | 210+39424 | 332 | 253 | 1113 | 7 |  |
| 22 | 39.4 | 42.7 | click 1.97 | 1242+39552 | 406 | 312 | 1264 | 8 | 52 KB image/jpeg 979x499 |
| 23 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 449.3 s, tool total 19.3 s over 23 turns; gate: 19 actions exec 11.9 s, gate decision 0.85 s, 17 observations 5.6 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow10-vlc-d06f0d4d-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 4.8 | 5.0 | set_plan 0.00, list_apps 0.03 | 335+4992 | 145 | 0 | 0 | 0 |  |
| 2 | 2.4 | 5.4 | run_command 0.03, list_windows 0.03 | 1849+5248 | 452 | 349 | 1288 | 0 |  |
| 3 | 2.5 | 4.2 | run_command 0.59, get_window_state 0.78 | 739+7040 | 253 | 101 | 304 | 0 |  |
| 4 | 1.7 | 3.4 | run_command 0.06 | 1140+7680 | 265 | 202 | 720 | 1 | 16 KB image/jpeg 600x420 |
| 5 | 2.0 | 3.7 | press_key 0.03 | 873+8704 | 313 | 215 | 848 | 1 |  |
| 6 | 2.6 | 2.8 | list_windows 0.01 | 472+9472 | 45 | 0 | 0 | 1 |  |
| 7 | 2.7 | 3.0 | get_window_state 0.31 | 207+9856 | 64 | 0 | 0 | 1 |  |
| 8 | 2.3 | 3.3 | click 1.03 | 3874+9984 | 69 | 21 | 81 | 2 | 59 KB image/jpeg 800x700 |
| 9 | 5.5 | 6.0 | get_window_state 0.21 | 156+13824 | 99 | 0 | 0 | 2 |  |
| 10 | 5.7 | 6.0 | get_window_state 0.19 | 172+13952 | 120 | 20 | 101 | 2 |  |
| 11 | 5.6 | 6.4 | click 0.41 | 3542+14080 | 138 | 44 | 183 | 2 |  |
| 12 | 17.2 | 17.7 | click 1.84 | 776+17536 | 94 | 0 | 0 | 3 | 59 KB image/jpeg 800x700 |
| 13 | 11.3 | 11.8 | get_window_state 0.30 | 201+18304 | 117 | 0 | 0 | 3 |  |
| 14 | 10.4 | 13.4 | run_actions 2.95 | 603+18432 | 443 | 318 | 1382 | 4 | 59 KB image/jpeg 800x700 |
| 15 | 3.1 | 4.5 | run_actions 1.39 | 2590+18944 | 232 | 123 | 459 | 4 |  |
| 16 | 14.0 | 14.2 | get_window_state 0.45 | 418+21504 | 64 | 0 | 0 | 4 |  |
| 17 | 8.4 | 9.1 | click 1.68 | 2413+21888 | 125 | 31 | 114 | 5 | 50 KB image/jpeg 800x700 |
| 18 | 22.6 | 23.2 | type_text 0.99 | 333+24192 | 62 | 0 | 0 | 5 |  |
| 19 | 5.8 | 6.0 | get_window_state 0.35 | 234+24448 | 100 | 0 | 0 | 5 |  |
| 20 | 14.0 | 14.5 | run_actions 3.07 | 2194+24576 | 171 | 48 | 210 | 6 | 37 KB image/jpeg 800x700 |
| 21 | 11.2 | 11.4 | click 0.04 | 733+26752 | 49 | 0 | 0 | 6 |  |
| 22 | 17.5 | 17.6 | get_window_state 0.37 | 253+27392 | 100 | 0 | 0 | 6 |  |
| 23 | 32.6 | 33.2 | click 1.50 | 2399+27520 | 132 | 38 | 154 | 7 | 41 KB image/jpeg 800x700 |
| 24 | 22.3 | 22.8 | get_window_state 0.22 | 326+29824 | 100 | 0 | 0 | 7 |  |
| 25 | 25.8 | 26.1 | click 1.41 | 28061+4096 | 94 | 0 | 0 | 8 | 44 KB image/jpeg 800x700 |
| 26 | 37.6 | 38.8 | get_window_state 0.19 | 2270+30080 | 100 | 0 | 0 | 8 |  |
| 27 | 34.1 | 34.8 | click 1.42 | 2191+32256 | 94 | 0 | 0 | 9 | 44 KB image/jpeg 800x700 |
| 28 | 42.5 | 43.1 | get_window_state 0.34 | 208+34432 | 98 | 0 | 0 | 9 |  |
| 29 | 23.8 | 29.3 | run_actions 5.07 | 591+34560 | 1012 | 832 | 2900 | 10 | 63 KB image/jpeg 800x700 |
| 30 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 420.8 s, tool total 27.3 s over 30 turns; gate: 22 actions exec 20.6 s, gate decision 0.11 s, 18 observations 4.4 s (10 with screenshot), reviewer calls 0 (0.0 s)

## ow10-vlc-d06f0d4d-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.6 | 4.6 | set_plan 0.00, list_apps 0.02 | 79+5248 | 158 | 58 | 250 | 0 |  |
| 2 | 1.8 | 2.4 | list_windows 0.01 | 540+5248 | 58 | 13 | 45 | 0 |  |
| 3 | 2.3 | 2.4 | get_window_state 0.20 | 132+5760 | 64 | 0 | 0 | 0 |  |
| 4 | 3.5 | 7.5 | run_command 0.03 | 720+5888 | 579 | 477 | 1842 | 1 | 16 KB image/jpeg 600x420 |
| 5 | 2.0 | 6.9 | run_command 0.02 | 1257+6528 | 744 | 635 | 2479 | 1 |  |
| 6 | 5.6 | 5.6 | run_command 0.04 | 1096+7680 | 137 | 38 | 150 | 1 |  |
| 7 | 3.3 | 4.2 | run_command 0.14 | 317+8704 | 175 | 104 | 392 | 1 |  |
| 8 | 2.7 | 3.2 | run_command 0.12 | 919+8960 | 124 | 32 | 117 | 1 |  |
| 9 | 4.4 | 6.4 | press_key 0.38 | 473+9856 | 579 | 481 | 1627 | 1 |  |
| 10 | 3.6 | 4.8 | click 1.34 | 4065+10240 | 177 | 129 | 573 | 1 |  |
| 11 | 2.6 | 3.1 | get_window_state 0.28 | 379+14208 | 119 | 19 | 78 | 1 |  |
| 12 | 7.1 | 8.4 | click 1.32 | 3988+14464 | 236 | 142 | 549 | 2 | 59 KB image/jpeg 800x700 |
| 13 | 14.0 | 15.9 | run_actions 1.27 | 806+18432 | 280 | 152 | 574 | 3 | 59 KB image/jpeg 800x700 |
| 14 | 7.5 | 10.3 | run_command 0.02 | 894+19200 | 442 | 352 | 1317 | 4 | 59 KB image/jpeg 800x700 |
| 15 | 16.9 | 17.6 | click 1.27 | 684+19968 | 167 | 73 | 253 | 4 |  |
| 16 | 10.1 | 13.6 | press_key 0.89 | 761+20608 | 554 | 443 | 1842 | 5 | 59 KB image/jpeg 800x700 |
| 17 | 25.8 | 27.2 | run_actions 1.37 | 1149+21248 | 305 | 171 | 781 | 6 | 59 KB image/jpeg 800x700 |
| 18 | 17.9 | 19.2 | get_window_state 0.00 | 2492+22272 | 196 | 113 | 475 | 6 |  |
| 19 | 7.7 | 7.9 | get_window_state 0.09 | 341+24704 | 99 | 0 | 0 | 6 |  |
| 20 | 4.1 | 4.6 | click 2.54 | 590+24960 | 126 | 32 | 109 | 7 | 50 KB image/jpeg 800x700 |
| 21 | 34.8 | 35.2 | type_text 0.92 | 726+25472 | 80 | 0 | 0 | 8 | 51 KB image/jpeg 800x700 |
| 22 | 30.5 | 30.6 | get_window_state 0.09 | 236+26112 | 99 | 0 | 0 | 8 |  |
| 23 | 32.0 | 32.6 | click 2.02 | 615+26240 | 108 | 14 | 62 | 9 | 39 KB image/jpeg 800x700 |
| 24 | 39.3 | 43.9 | run_actions 2.37 | 5619+26752 | 916 | 725 | 2291 | 10 | 57 KB image/jpeg 800x700 |
| 25 | 43.4 | 44.4 | run_actions 2.49 | 1250+32256 | 165 | 22 | 107 | 10 |  |
| 26 | 47.6 | 48.3 | click 1.18 | 733+33408 | 128 | 79 | 352 | 10 |  |
| 27 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 410.7 s, tool total 20.4 s over 27 turns; gate: 23 actions exec 14.5 s, gate decision 0.36 s, 17 observations 4.5 s (10 with screenshot), reviewer calls 0 (0.0 s)

## ow11-libreoffice_writer-0e47de2a-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.9 | 4.4 | set_plan 0.00, list_windows 0.01 | 388+4992 | 120 | 0 | 0 | 0 |  |
| 2 | 2.2 | 2.4 | get_window_state 0.38 | 372+5376 | 118 | 54 | 245 | 0 |  |
| 3 | 3.0 | 3.9 | click 0.05 | 4269+5632 | 195 | 147 | 632 | 1 | 75 KB image/jpeg 1396x823 |
| 4 | 2.7 | 3.0 | get_window_state 0.38 | 311+9856 | 63 | 0 | 0 | 1 |  |
| 5 | 16.5 | 17.3 | press_key 0.18 | 874+10112 | 173 | 75 | 350 | 2 | 75 KB image/jpeg 1396x823 |
| 6 | 8.3 | 8.8 | get_window_state 0.26 | 343+10880 | 79 | 0 | 0 | 2 |  |
| 7 | 13.3 | 13.7 | click 0.03 | 332+11136 | 96 | 48 | 195 | 2 |  |
| 8 | 24.9 | 25.0 | get_window_state 0.25 | 287+11392 | 83 | 19 | 84 | 2 |  |
| 9 | 4.2 | 11.5 | click 1.54 | 372+11648 | 1265 | 1171 | 4819 | 3 | 28 KB image/jpeg 357x695 |
| 10 | 6.1 | 8.4 | run_actions 2.88 | 1527+11904 | 319 | 170 | 643 | 3 |  |
| 11 | 8.8 | 8.9 | get_window_state 0.31 | 2849+13312 | 126 | 45 | 199 | 3 |  |
| 12 | 16.0 | 17.1 | get_window_state 0.25 | 396+16128 | 173 | 91 | 335 | 3 |  |
| 13 | 10.8 | 12.5 | run_actions 0.61 | 5555+16512 | 260 | 147 | 626 | 4 | 4 KB image/jpeg 228x87 |
| 14 | 11.9 | 12.8 | press_key 0.10 | 2642+22016 | 170 | 91 | 414 | 4 |  |
| 15 | 17.7 | 18.0 | get_window_state 0.42 | 316+24576 | 45 | 0 | 0 | 4 |  |
| 16 | 22.5 | 25.3 | run_actions 2.99 | 4092+24832 | 534 | 333 | 1368 | 5 | 82 KB image/jpeg 1396x823 |
| 17 | 30.2 | 31.7 | get_window_state 0.19 | 1202+28800 | 182 | 118 | 546 | 5 |  |
| 18 | 30.7 | 32.6 | run_actions 1.21 | 981+29952 | 287 | 156 | 608 | 6 | 82 KB image/jpeg 1396x823 |
| 19 | 23.8 | 24.5 | get_window_state 0.18 | 545+30848 | 64 | 0 | 0 | 6 |  |
| 20 | 29.4 | 34.1 | run_actions 3.03 | 848+31360 | 235 | 109 | 379 | 7 | 82 KB image/jpeg 1396x823 |
| 21 | 40.7 | 42.9 | run_actions 2.04 | 31777+4096 | 449 | 327 | 1303 | 8 | 73 KB image/jpeg 1396x823 |
| 22 | 44.4 | 46.1 | run_actions 1.19 | 7108+32128 | 351 | 246 | 1023 | 8 |  |
| 23 | 46.2 | 48.2 | run_actions 1.21 | 3232+39168 | 332 | 227 | 973 | 8 |  |
| 24 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 453.2 s, tool total 19.7 s over 24 turns; gate: 25 actions exec 13.8 s, gate decision 0.11 s, 18 observations 4.7 s (8 with screenshot), reviewer calls 0 (0.0 s)

## ow11-libreoffice_writer-0e47de2a-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.5 | set_plan 0.00, list_windows 0.02 | 4+5376 | 125 | 11 | 48 | 0 |  |
| 2 | 3.5 | 4.0 | get_window_state 0.11 | 381+5376 | 107 | 7 | 31 | 0 |  |
| 3 | 1.7 | 2.3 | click 0.62 | 2451+5632 | 167 | 102 | 425 | 0 |  |
| 4 | 2.2 | 3.9 | get_window_state 0.34 | 2453+8064 | 288 | 208 | 908 | 0 |  |
| 5 | 3.6 | 4.2 | click 2.87 | 691+10496 | 75 | 10 | 37 | 1 | 28 KB image/jpeg 357x695 |
| 6 | 5.2 | 5.8 | click 0.89 | 511+11136 | 100 | 35 | 148 | 1 |  |
| 7 | 7.0 | 7.9 | set_plan 0.16, get_window_state 0.25 | 323+11520 | 226 | 37 | 185 | 1 |  |
| 8 | 4.8 | 5.9 | scroll 3.55 | 2578+11776 | 257 | 115 | 446 | 2 | 73 KB image/jpeg 1396x823 |
| 9 | 10.6 | 11.2 | run_command 0.03 | 3730+14336 | 179 | 99 | 390 | 3 | 76 KB image/jpeg 1396x823 |
| 10 | 10.9 | 11.4 | scroll 3.45 | 431+18048 | 177 | 35 | 129 | 3 |  |
| 11 | 4.4 | 6.0 | scroll 3.45 | 3679+18432 | 251 | 109 | 475 | 4 | 96 KB image/jpeg 1396x823 |
| 12 | 18.7 | 21.6 | press_key 1.31 | 3910+22016 | 418 | 320 | 1434 | 5 | 83 KB image/jpeg 1396x823 |
| 13 | 26.5 | 27.1 | run_command 0.01 | 1911+25856 | 173 | 20 | 86 | 6 | 83 KB image/jpeg 1396x823 |
| 14 | 35.4 | 36.1 | run_command 0.12 | 23956+4096 | 174 | 0 | 0 | 6 |  |
| 15 | 18.0 | 22.5 | run_command 0.09 | 1296+27648 | 798 | 618 | 2723 | 6 |  |
| 16 | 27.9 | 29.0 | get_window_state 0.22 | 1061+28928 | 155 | 54 | 245 | 6 |  |
| 17 | 36.3 | 38.8 | run_command 2.52 | 941+29952 | 396 | 233 | 973 | 7 | 83 KB image/jpeg 1396x823 |
| 18 | 39.0 | 41.8 | run_actions 0.02 | 596+30848 | 518 | 415 | 1735 | 7 |  |
| 19 | 37.9 | 38.4 | run_actions 0.32 | 726+31360 | 119 | 0 | 0 | 7 |  |
| 20 | 41.3 | 41.7 | click 0.36 | 297+32000 | 92 | 27 | 101 | 7 |  |
| 21 | 47.7 | 47.9 | click 1.37 | 270+32256 | 66 | 0 | 0 | 7 |  |
| 22 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 410.0 s, tool total 22.1 s over 22 turns; gate: 15 actions exec 15.5 s, gate decision 1.61 s, 13 observations 3.6 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow12-libreoffice_calc-8b1ce5f2-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.5 | list_apps 0.06, list_windows 0.06 | 1371+4096 | 59 | 13 | 57 | 0 |  |
| 2 | 1.8 | 2.6 | set_plan 0.00, get_window_state 0.39 | 1817+5376 | 159 | 10 | 43 | 0 |  |
| 3 | 10.6 | 15.3 | run_command 0.25 | 5685+7168 | 793 | 654 | 2968 | 1 | 166 KB image/jpeg 1396x823 |
| 4 | 12.6 | 16.7 | run_command 0.10 | 2887+12800 | 752 | 487 | 2145 | 1 |  |
| 5 | 13.2 | 18.0 | run_command 0.13 | 993+15616 | 869 | 601 | 2614 | 1 |  |
| 6 | 19.4 | 34.6 | run_actions 0.92 | 1326+16512 | 222 | 101 | 435 | 1 |  |
| 7 | 12.9 | 12.9 |  | 0+0 | 0 | None | 0 | 1 |  |
| 8 | 16.6 | 16.8 | click 0.89 | 2724+17792 | 48 | 0 | 0 | 1 |  |
| 9 | 15.3 | 16.3 | list_windows 0.01 | 129+20480 | 62 | 17 | 85 | 1 |  |
| 10 | 17.2 | 17.6 | get_window_state 0.39 | 124+20608 | 82 | 0 | 0 | 1 |  |
| 11 | 26.9 | 28.4 | press_key 0.90 | 3793+20608 | 266 | 168 | 570 | 2 | 176 KB image/jpeg 1396x823 |
| 12 | 23.0 | 23.4 | get_window_state 0.19 | 413+24320 | 101 | 0 | 0 | 2 |  |
| 13 | 47.3 | 53.1 | run_command 0.31 | 879+24704 | 1120 | 862 | 2903 | 3 | 179 KB image/jpeg 1396x823 |
| 14 | 46.3 | 46.7 | check_done 0.33, set_plan 0.33 | 1302+25472 | 105 | 0 | 0 | 3 |  |
| 15 | 53.6 | 56.2 | press_key 0.86 | 1542+26752 | 320 | 222 | 968 | 3 |  |
| 16 | 49.7 | 50.1 | get_window_state 0.24 | 390+28288 | 101 | 0 | 0 | 3 |  |
| 17 | 52.0 | 53.0 | run_command 0.12 | 856+28672 | 253 | 15 | 78 | 4 | 179 KB image/jpeg 1396x823 |
| 18 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 464.3 s, tool total 6.5 s over 18 turns; gate: 9 actions exec 3.8 s, gate decision 0.03 s, 7 observations 1.2 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow12-libreoffice_calc-8b1ce5f2-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.0 | 3.0 | set_plan 0.00, list_apps 0.03 | 1371+4096 | 138 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.2 | list_windows 0.01 | 544+5376 | 59 | 13 | 49 | 0 |  |
| 3 | 1.6 | 2.0 | get_window_state 0.54 | 152+5888 | 100 | 0 | 0 | 0 |  |
| 4 | 18.0 | 19.6 | run_command 0.13 | 5568+6016 | 413 | 261 | 1067 | 1 | 166 KB image/jpeg 1396x823 |
| 5 | 16.0 | 19.8 | run_command 0.10 | 8535+5376 | 763 | 600 | 2599 | 1 |  |
| 6 | 12.9 | 15.3 | run_command 0.39 | 3332+11520 | 430 | 197 | 803 | 1 |  |
| 7 | 15.3 | 16.1 | click 0.86 | 624+14848 | 141 | 57 | 213 | 1 |  |
| 8 | 21.1 | 21.6 | click 2.25 | 2701+15360 | 84 | 0 | 0 | 1 |  |
| 9 | 13.1 | 13.8 | get_window_state 0.25 | 4294+18048 | 134 | 14 | 60 | 1 |  |
| 10 | 24.5 | 27.4 | run_command 0.13 | 628+22272 | 566 | 328 | 1087 | 2 | 100 KB image/jpeg 1000x590 |
| 11 | 23.0 | 24.1 | press_key 1.31 | 749+22784 | 149 | 51 | 215 | 2 |  |
| 12 | 16.0 | 17.0 | get_window_state 0.15 | 770+23424 | 119 | 0 | 0 | 2 |  |
| 13 | 33.1 | 35.1 | run_command 0.14 | 469+24192 | 400 | 229 | 919 | 3 | 83 KB image/jpeg 900x531 |
| 14 | 45.3 | 45.4 | set_plan 0.00 | 788+24576 | 157 | 102 | 393 | 3 |  |
| 15 | 34.2 | 34.6 | check_done 0.13 | 258+25344 | 60 | 0 | 0 | 3 |  |
| 16 | 37.4 | 37.8 |  | 1365+25600 | 61 | 0 | 0 | 3 |  |

model total 334.9 s, tool total 6.4 s over 16 turns; gate: 8 actions exec 2.9 s, gate decision 0.50 s, 7 observations 2.5 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow13-libreoffice_impress-986fc832-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.0 | 2.7 | list_windows 0.01, set_plan 0.01 | 411+4992 | 130 | 0 | 0 | 0 |  |
| 2 | 2.4 | 2.8 | get_window_state 0.45 | 410+5376 | 89 | 9 | 33 | 0 |  |
| 3 | 10.1 | 21.6 | run_command 0.04, run_command 0.07 | 1768+5760 | 1796 | 1638 | 6576 | 1 | 118 KB image/jpeg 1396x823 |
| 4 | 13.4 | 15.7 | run_command 0.02 | 2860+7424 | 503 | 430 | 1809 | 1 |  |
| 5 | 4.8 | 7.4 | run_actions 5.04 | 775+10240 | 468 | 300 | 1145 | 1 |  |
| 6 | 8.3 | 9.2 | run_actions 2.26 | 3839+11008 | 285 | 156 | 670 | 2 | 122 KB image/jpeg 1396x823 |
| 7 | 33.1 | 34.7 | run_actions 0.02 | 2150+14720 | 247 | 136 | 572 | 2 |  |
| 8 | 20.2 | 20.7 | run_actions 0.30 | 474+16768 | 80 | 0 | 0 | 2 |  |
| 9 | 27.4 | 27.5 | list_windows 0.01 | 287+17152 | 46 | 0 | 0 | 2 |  |
| 10 | 20.9 | 23.7 | run_actions 1.38 | 133+17408 | 463 | 333 | 1374 | 2 |  |
| 11 | 37.4 | 38.0 | run_actions 0.03 | 3158+17536 | 113 | 29 | 138 | 3 | 117 KB image/jpeg 1396x823 |
| 12 | 18.4 | 19.0 | run_actions 1.83 | 328+20608 | 140 | 38 | 130 | 3 |  |
| 13 | 20.0 | 20.9 | run_actions 1.92 | 3530+20864 | 179 | 77 | 312 | 4 | 38 KB image/jpeg 622x551 |
| 14 | 36.6 | 39.3 | run_actions 1.19 | 3296+24320 | 323 | 221 | 911 | 5 | 36 KB image/jpeg 622x551 |
| 15 | 44.0 | 45.7 | run_command 0.02 | 925+27520 | 342 | 251 | 1036 | 6 | 45 KB image/jpeg 622x551 |
| 16 | 45.0 | 48.6 | run_command 0.05 | 1210+28416 | 597 | 532 | 1787 | 6 |  |
| 17 | 24.2 | 27.9 | run_actions 2.43 | 775+29568 | 662 | 548 | 1788 | 6 |  |
| 18 | 30.3 | 34.0 | run_actions 3.10 | 1188+30336 | 681 | 567 | 1822 | 7 | 38 KB image/jpeg 622x551 |
| 19 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 439.5 s, tool total 20.2 s over 19 turns; gate: 19 actions exec 11.1 s, gate decision 0.06 s, 11 observations 5.0 s (8 with screenshot), reviewer calls 0 (0.0 s)

## ow13-libreoffice_impress-986fc832-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.5 | 3.4 | set_plan 0.00, list_apps 0.02 | 27+5376 | 139 | 0 | 0 | 0 |  |
| 2 | 10.8 | 11.0 | list_windows 0.01 | 486+5376 | 54 | 9 | 33 | 0 |  |
| 3 | 1.7 | 2.1 | get_window_state 0.44 | 212+5760 | 83 | 0 | 0 | 0 |  |
| 4 | 10.9 | 14.1 | click 1.66 | 4998+5888 | 562 | 452 | 1875 | 1 | 116 KB image/jpeg 1396x823 |
| 5 | 5.9 | 9.1 | press_key 1.48 | 3784+10880 | 431 | 333 | 1320 | 2 | 119 KB image/jpeg 1396x823 |
| 6 | 20.2 | 20.7 | click 2.16 | 3694+14592 | 92 | 44 | 186 | 3 | 119 KB image/jpeg 1396x823 |
| 7 | 12.9 | 14.5 | click 1.15 | 2196+18176 | 256 | 208 | 867 | 4 | 45 KB image/jpeg 804x607 |
| 8 | 25.5 | 26.3 | get_window_state 0.27 | 369+20352 | 224 | 142 | 653 | 4 |  |
| 9 | 27.0 | 31.7 | press_key 0.74 | 2395+20608 | 807 | 709 | 2700 | 5 | 119 KB image/jpeg 1396x823 |
| 10 | 45.5 | 46.0 | click 1.65 | 2846+22912 | 60 | 12 | 47 | 5 |  |
| 11 | 28.2 | 28.4 | get_window_state 0.41 | 3208+25728 | 198 | 81 | 370 | 5 |  |
| 12 | 52.7 | 53.3 | get_window_state 0.49 | 344+28928 | 129 | 30 | 139 | 5 |  |
| 13 | 33.4 | 33.9 | click 1.92 | 374+29184 | 75 | 26 | 113 | 5 |  |
| 14 | 46.1 | 47.1 | click 1.14 | 2963+29440 | 191 | 142 | 536 | 5 |  |
| 15 | 41.7 | 42.2 | get_window_state 0.31 | 500+32384 | 106 | 24 | 77 | 5 |  |
| 16 | 23.4 | 23.6 | click 5.30 | 1101+32768 | 49 | 0 | 0 | 6 | 12 KB image/jpeg 185x393 |
| 17 | 32.0 | 33.2 | press_key 1.25 | 343+33792 | 180 | 120 | 497 | 6 |  |
| 18 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 440.4 s, tool total 20.4 s over 18 turns; gate: 10 actions exec 11.2 s, gate decision 1.60 s, 15 observations 6.8 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow14-libreoffice_writer-72b810ef-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.6 | 3.9 | set_plan 0.00, list_windows 0.01 | 32+5376 | 130 | 0 | 0 | 0 |  |
| 2 | 4.2 | 4.4 | get_window_state 0.40 | 419+5376 | 64 | 0 | 0 | 0 |  |
| 3 | 7.9 | 12.0 | run_actions 2.21 | 4594+5760 | 621 | 467 | 2103 | 1 | 129 KB image/jpeg 1396x823 |
| 4 | 13.2 | 13.8 | click 0.02 | 2180+10240 | 305 | 257 | 1065 | 1 |  |
| 5 | 12.8 | 15.6 | click 0.92 | 412+12416 | 422 | 374 | 1501 | 1 |  |
| 6 | 17.5 | 17.5 | get_window_state 0.31 | 560+12800 | 98 | 0 | 0 | 1 |  |
| 7 | 15.0 | 15.3 | click 0.02 | 478+13312 | 48 | 0 | 0 | 1 |  |
| 8 | 20.2 | 22.2 | run_actions 3.99 | 247+13696 | 809 | 576 | 2517 | 1 |  |
| 9 | 5.6 | 6.1 | get_window_state 0.56 | 1421+13824 | 103 | 23 | 102 | 1 |  |
| 10 | 3.2 | 3.8 | get_window_state 0.44 | 1102+15232 | 99 | 18 | 76 | 2 | 97 KB image/jpeg 1396x823 |
| 11 | 22.7 | 24.0 | click 1.56 | 551+16256 | 231 | 137 | 538 | 3 | 37 KB image/jpeg 622x551 |
| 12 | 28.3 | 28.6 | get_window_state 0.30 | 362+16768 | 64 | 0 | 0 | 3 |  |
| 13 | 20.3 | 20.7 | click 0.72 | 5456+17024 | 72 | 24 | 91 | 4 | 35 KB image/jpeg 622x551 |
| 14 | 29.2 | 29.3 | get_window_state 0.53 | 262+22400 | 98 | 0 | 0 | 4 |  |
| 15 | 31.7 | 31.9 | click 1.37 | 450+22656 | 48 | 0 | 0 | 5 | 4 KB image/jpeg 185x140 |
| 16 | 33.5 | 34.0 | press_key 0.89 | 246+23040 | 118 | 39 | 169 | 5 |  |
| 17 | 37.1 | 37.4 | get_window_state 0.08 | 19375+4096 | 83 | 0 | 0 | 5 |  |
| 18 | 25.9 | 26.4 | press_key 0.11 | 712+23168 | 133 | 54 | 246 | 6 | 35 KB image/jpeg 622x551 |
| 19 | 31.6 | 32.1 | get_window_state 0.09 | 272+23808 | 83 | 0 | 0 | 6 |  |
| 20 | 62.9 | 63.2 | click 0.80 | 372+24064 | 70 | 22 | 86 | 7 | 31 KB image/jpeg 622x551 |
| 21 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 442.1 s, tool total 15.3 s over 21 turns; gate: 21 actions exec 11.1 s, gate decision 0.09 s, 11 observations 3.4 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow14-libreoffice_writer-72b810ef-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.1 | 3.0 | set_plan 0.00, list_windows 0.03 | 32+5376 | 128 | 0 | 0 | 0 |  |
| 2 | 2.5 | 3.9 | get_window_state 0.08, run_command 0.10 | 418+5376 | 156 | 14 | 64 | 0 |  |
| 3 | 3.1 | 3.9 | run_command 0.02 | 2513+5760 | 144 | 62 | 295 | 0 |  |
| 4 | 2.3 | 3.1 | run_command 0.15 | 491+8192 | 148 | 0 | 0 | 0 |  |
| 5 | 3.9 | 6.1 | run_command 0.01 | 502+8576 | 226 | 108 | 494 | 0 |  |
| 6 | 1.8 | 2.2 | run_command 0.04 | 5322+4096 | 115 | 0 | 0 | 0 |  |
| 7 | 2.0 | 3.2 | run_command 0.02 | 1669+8960 | 183 | 111 | 506 | 0 |  |
| 8 | 3.2 | 6.6 | get_window_state 0.21 | 1961+10624 | 569 | 486 | 2068 | 0 |  |
| 9 | 15.1 | 17.7 | run_actions 3.79 | 1359+12544 | 636 | 449 | 1690 | 1 | 129 KB image/jpeg 1396x823 |
| 10 | 17.6 | 18.6 | click 1.37 | 4122+13824 | 155 | 107 | 436 | 1 |  |
| 11 | 7.5 | 8.1 | click 1.26 | 2166+17920 | 138 | 90 | 373 | 1 |  |
| 12 | 18.5 | 19.3 | get_window_state 0.21 | 3587+19968 | 137 | 54 | 245 | 1 |  |
| 13 | 59.3 | 60.2 | run_actions 1.19 | 889+23552 | 193 | 88 | 355 | 2 | 131 KB image/jpeg 1396x823 |
| 14 | 23.3 | 24.2 | run_command 0.02, list_windows 0.03 | 1146+24320 | 120 | 39 | 173 | 2 |  |
| 15 | 14.4 | 15.0 | run_command 0.08 | 566+25344 | 152 | 40 | 134 | 2 |  |
| 16 | 28.8 | 28.9 | run_command 0.08, set_plan 0.08 | 306+25856 | 174 | 20 | 109 | 2 |  |
| 17 | 21.0 | 21.6 | check_done 0.09 | 394+26112 | 65 | 0 | 0 | 2 |  |
| 18 | 16.4 | 16.8 |  | 367+26496 | 68 | 0 | 0 | 2 |  |

model total 262.4 s, tool total 8.9 s over 18 turns; gate: 15 actions exec 5.7 s, gate decision 0.59 s, 9 observations 1.8 s (2 with screenshot), reviewer calls 0 (0.0 s)

## ow15-libreoffice_impress-a097acff-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.7 | 4.3 | set_plan 0.00, list_windows 0.01 | 112+5248 | 99 | 0 | 0 | 0 |  |
| 2 | 1.9 | 2.5 | press_key 0.11 | 456+5248 | 147 | 30 | 109 | 0 |  |
| 3 | 1.7 | 1.9 | list_windows 0.01 | 286+5632 | 45 | 0 | 0 | 0 |  |
| 4 | 1.3 | 1.6 | get_window_state 0.41 | 163+5888 | 82 | 0 | 0 | 0 |  |
| 5 | 2.0 | 2.6 | run_command 0.02, click 1.45 | 1400+6016 | 211 | 114 | 465 | 0 |  |
| 6 | 1.9 | 2.6 | run_actions 2.52 | 476+7296 | 164 | 15 | 70 | 0 |  |
| 7 | 2.8 | 2.9 | click 0.71 | 547+7680 | 167 | 119 | 455 | 0 |  |
| 8 | 1.3 | 1.8 | get_window_state 0.32 | 310+8192 | 99 | 0 | 0 | 0 |  |
| 9 | 1.8 | 1.9 | click 1.43 | 491+8448 | 48 | 0 | 0 | 0 |  |
| 10 | 3.3 | 6.3 | press_key 1.54 | 280+8832 | 476 | 397 | 1686 | 0 |  |
| 11 | 2.0 | 2.3 | get_window_state 0.09 | 627+9088 | 83 | 0 | 0 | 0 |  |
| 12 | 3.5 | 4.8 | run_actions 0.53 | 540+9600 | 156 | 38 | 147 | 1 | 50 KB image/jpeg 782x497 |
| 13 | 2.9 | 3.7 | click 0.81 | 331+10112 | 48 | 0 | 0 | 1 |  |
| 14 | 2.8 | 3.1 | run_command 2.02 | 218+10368 | 55 | 0 | 0 | 1 |  |
| 15 | 6.3 | 6.8 | list_windows 0.00 | 232+10496 | 85 | 39 | 178 | 1 |  |
| 16 | 9.1 | 9.5 | set_plan 0.00, check_done 0.09 | 243+10624 | 97 | 0 | 0 | 1 |  |
| 17 | 4.3 | 4.7 |  | 500+10752 | 67 | 0 | 0 | 1 |  |

model total 63.3 s, tool total 12.1 s over 17 turns; gate: 11 actions exec 10.1 s, gate decision 0.05 s, 8 observations 1.7 s (1 with screenshot), reviewer calls 0 (0.0 s)

## ow15-libreoffice_impress-a097acff-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.4 | 4.5 | set_plan 0.00, list_windows 0.01 | 112+5248 | 132 | 0 | 0 | 0 |  |
| 2 | 7.6 | 7.7 | press_key 0.42 | 506+5248 | 125 | 24 | 86 | 0 |  |
| 3 | 1.7 | 2.6 | click 0.73 | 2703+5632 | 118 | 70 | 309 | 0 |  |
| 4 | 3.5 | 3.7 | click 1.99 | 2640+8320 | 48 | 0 | 0 | 0 |  |
| 5 | 9.0 | 9.9 | run_command 0.03 | 1490+10880 | 271 | 193 | 802 | 0 |  |
| 6 | 7.3 | 7.5 | run_actions 4.01 | 521+12288 | 217 | 56 | 230 | 0 |  |
| 7 | 3.3 | 4.1 | get_window_state 0.25 | 561+12800 | 165 | 66 | 333 | 0 |  |
| 8 | 5.2 | 6.0 | get_window_state 0.09 | 410+13312 | 134 | 33 | 148 | 0 |  |
| 9 | 3.1 | 3.5 | click 0.97 | 449+13696 | 69 | 21 | 69 | 1 | 37 KB image/jpeg 782x497 |
| 10 | 2.9 | 3.4 | run_command 0.02, list_windows 0.03 | 233+14080 | 107 | 21 | 97 | 1 |  |
| 11 | 4.3 | 5.4 | run_command 0.08, set_plan 0.08 | 426+14208 | 246 | 91 | 372 | 1 |  |
| 12 | 3.5 | 4.0 | check_done 0.08 | 486+14592 | 52 | 0 | 0 | 1 |  |
| 13 | 3.6 | 3.9 | set_plan 0.00 | 404+14976 | 55 | 0 | 0 | 1 |  |
| 14 | 5.0 | 5.5 |  | 152+15360 | 83 | 0 | 0 | 1 |  |

model total 71.6 s, tool total 8.8 s over 14 turns; gate: 10 actions exec 5.5 s, gate decision 0.72 s, 8 observations 2.1 s (1 with screenshot), reviewer calls 0 (0.0 s)

## ow16-libreoffice_calc-a9f325aa-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 4.5 | 5.5 | set_plan 0.01, list_windows 0.01 | 631+4864 | 128 | 0 | 0 | 0 |  |
| 2 | 4.9 | 5.0 | get_window_state 0.39 | 500+5376 | 90 | 8 | 36 | 0 |  |
| 3 | 12.0 | 16.0 | run_command 0.15 | 3696+5760 | 685 | 546 | 2379 | 1 | 130 KB image/jpeg 1396x823 |
| 4 | 10.1 | 14.3 | run_command 0.10 | 1325+9344 | 469 | 353 | 1393 | 1 |  |
| 5 | 10.9 | 18.2 | run_actions 4.28 | 788+10624 | 1143 | 874 | 3527 | 1 |  |
| 6 | 18.3 | 19.9 | run_actions 1.55 | 3266+11392 | 292 | 130 | 501 | 1 |  |
| 7 | 3.8 | 4.2 | click 0.76 | 1623+14592 | 84 | 0 | 0 | 1 |  |
| 8 | 12.1 | 13.7 | run_actions 1.32 | 273+16128 | 261 | 141 | 556 | 1 |  |
| 9 | 4.2 | 5.2 | click 0.83 | 13794+4096 | 198 | 114 | 516 | 1 |  |
| 10 | 6.6 | 6.7 | get_window_state 0.15 | 1794+16384 | 117 | 0 | 0 | 1 |  |
| 11 | 26.2 | 26.2 | run_actions 1.32 | 250+18176 | 152 | 32 | 118 | 1 |  |
| 12 | 17.2 | 18.2 | run_command 0.10 | 704+18304 | 225 | 76 | 332 | 1 |  |
| 13 | 16.2 | 17.5 | set_plan 0.00 | 610+18944 | 216 | 106 | 471 | 1 |  |
| 14 | 14.6 | 14.7 | check_done 0.11 | 385+19456 | 57 | 0 | 0 | 1 |  |
| 15 | 19.5 | 19.6 |  | 832+19840 | 79 | 0 | 0 | 1 |  |

model total 204.9 s, tool total 11.1 s over 15 turns; gate: 14 actions exec 8.4 s, gate decision 0.06 s, 7 observations 2.2 s (1 with screenshot), reviewer calls 0 (0.0 s)

## ow16-libreoffice_calc-a9f325aa-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.9 | 2.7 | set_plan 0.00, list_windows 0.01 | 119+5376 | 128 | 0 | 0 | 0 |  |
| 2 | 2.6 | 3.0 | get_window_state 0.55 | 497+5376 | 80 | 16 | 61 | 0 |  |
| 3 | 7.4 | 15.1 | run_command 0.13 | 4675+5760 | 1141 | 984 | 3963 | 1 | 130 KB image/jpeg 1396x823 |
| 4 | 2.8 | 4.7 | run_command 0.12 | 1774+10368 | 371 | 222 | 857 | 1 |  |
| 5 | 11.4 | 15.3 | set_plan 0.83, click 0.93 | 711+12032 | 627 | 494 | 1896 | 1 |  |
| 6 | 3.4 | 6.9 | run_actions 1.51 | 1790+12672 | 605 | 433 | 1669 | 2 | 131 KB image/jpeg 1396x823 |
| 7 | 22.4 | 23.1 | click 2.20 | 1652+14336 | 151 | 57 | 232 | 3 | 131 KB image/jpeg 1396x823 |
| 8 | 52.5 | 53.0 | type_text 0.86 | 1143+15872 | 100 | 20 | 64 | 4 | 131 KB image/jpeg 1396x823 |
| 9 | 20.3 | 21.5 | press_key 1.71 | 286+16896 | 79 | 0 | 0 | 4 |  |
| 10 | 59.4 | 63.8 | run_actions 6.19 | 1096+17152 | 730 | 467 | 2063 | 5 | 131 KB image/jpeg 1396x823 |
| 11 | 71.4 | 72.8 | run_actions 6.24 | 2494+18176 | 381 | 15 | 51 | 5 |  |
| 12 | 55.4 | 57.2 | run_actions 6.39 | 1314+20608 | 401 | 0 | 0 | 5 |  |
| 13 | 58.7 | 59.8 | run_actions 2.67 | 1395+21888 | 255 | 0 | 0 | 5 |  |
| 14 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 398.9 s, tool total 30.3 s over 14 turns; gate: 61 actions exec 23.8 s, gate decision 0.25 s, 10 observations 4.4 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow17-gimp-045bf3ff-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 3.2 | set_plan 0.00 | 55+5248 | 100 | 0 | 0 | 0 |  |
| 2 | 2.9 | 3.2 | list_windows 0.01, list_apps 0.03 | 214+5248 | 52 | 6 | 29 | 0 |  |
| 3 | 1.5 | 3.0 | get_window_state 0.36 | 1850+5376 | 314 | 250 | 995 | 0 |  |
| 4 | 4.5 | 5.6 | click 0.06 | 1331+7168 | 137 | 89 | 369 | 1 | 17 KB image/jpeg 427x318 |
| 5 | 4.2 | 4.7 | get_window_state 0.24 | 297+8448 | 98 | 16 | 62 | 1 |  |
| 6 | 2.9 | 3.2 | click 0.87 | 1054+8704 | 48 | 0 | 0 | 1 |  |
| 7 | 1.8 | 2.1 | get_window_state 0.40 | 122+9728 | 64 | 0 | 0 | 1 |  |
| 8 | 17.0 | 18.5 | click 1.11 | 4787+9728 | 96 | 48 | 157 | 2 | 114 KB image/jpeg 826x781 |
| 9 | 13.7 | 13.9 | list_windows 0.01 | 211+14464 | 45 | 0 | 0 | 2 |  |
| 10 | 23.1 | 23.5 | get_window_state 0.69 | 212+14592 | 102 | 23 | 95 | 2 |  |
| 11 | 16.1 | 16.3 | click 0.87 | 954+14720 | 61 | 13 | 46 | 3 | 17 KB image/jpeg 272x543 |
| 12 | 10.6 | 11.0 | get_window_state 0.07 | 183+15616 | 64 | 0 | 0 | 3 |  |
| 13 | 3.5 | 6.2 | press_key 0.12 | 384+15744 | 385 | 325 | 1405 | 4 | 2 KB image/jpeg 153x87 |
| 14 | 24.2 | 24.4 | click 1.15 | 450+16128 | 48 | 0 | 0 | 4 |  |
| 15 | 2.2 | 2.8 | get_window_state 0.38 | 238+16512 | 115 | 17 | 77 | 4 |  |
| 16 | 27.1 | 27.2 | click 0.68 | 447+16640 | 48 | 0 | 0 | 4 |  |
| 17 | 5.2 | 5.7 | get_window_state 0.07 | 218+17024 | 83 | 0 | 0 | 4 |  |
| 18 | 5.3 | 6.4 | press_key 0.11 | 397+17152 | 181 | 121 | 462 | 5 | 12 KB image/jpeg 248x464 |
| 19 | 16.3 | 17.8 | set_plan 0.00 | 259+17536 | 204 | 0 | 0 | 5 |  |
| 20 | 16.2 | 16.5 | check_done 0.02 | 335+17792 | 28 | 0 | 0 | 5 |  |
| 21 | 18.2 | 18.7 |  | 280+18048 | 86 | 0 | 0 | 5 |  |

model total 233.9 s, tool total 7.2 s over 21 turns; gate: 8 actions exec 4.9 s, gate decision 0.03 s, 9 observations 1.9 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow17-gimp-045bf3ff-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.9 | set_plan 0.00, list_windows 0.01 | 439+4864 | 136 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.6 | run_command 0.03 | 475+5248 | 82 | 20 | 86 | 0 |  |
| 3 | 4.4 | 8.3 | get_window_state 0.51 | 1079+5632 | 383 | 301 | 1211 | 0 |  |
| 4 | 2.1 | 3.2 | click 0.24 | 1397+6656 | 184 | 136 | 568 | 1 | 17 KB image/jpeg 427x318 |
| 5 | 3.7 | 4.2 | get_window_state 0.16 | 409+7936 | 132 | 32 | 122 | 1 |  |
| 6 | 2.9 | 3.1 | click 1.86 | 1338+8320 | 48 | 0 | 0 | 2 | 39 KB image/jpeg 647x606 |
| 7 | 5.8 | 6.4 | get_window_state 0.28 | 5653+4096 | 121 | 21 | 87 | 2 |  |
| 8 | 10.4 | 11.8 | click 2.41 | 3640+9600 | 222 | 174 | 649 | 3 | 114 KB image/jpeg 826x781 |
| 9 | 17.5 | 19.5 | run_command 0.06, press_key 2.16 | 3115+13184 | 370 | 196 | 777 | 3 |  |
| 10 | 23.8 | 28.8 | run_command 0.02 | 9708+9728 | 313 | 221 | 927 | 3 |  |
| 11 | 7.8 | 9.3 | run_actions 4.08 | 3712+16256 | 293 | 129 | 537 | 3 |  |
| 12 | 6.9 | 7.4 | click 1.12 | 994+19968 | 91 | 43 | 170 | 3 |  |
| 13 | 9.9 | 10.4 | click 1.88 | 623+20864 | 122 | 73 | 294 | 3 |  |
| 14 | 4.5 | 6.3 | click 1.96 | 1351+21376 | 127 | 78 | 285 | 3 |  |
| 15 | 9.8 | 24.7 | run_actions 6.29 | 1056+22656 | 164 | 0 | 0 | 3 |  |
| 16 | 11.2 | 12.0 | list_windows 0.01 | 2871+23680 | 108 | 63 | 268 | 3 |  |
| 17 | 4.7 | 5.1 | get_window_state 0.12 | 278+26496 | 100 | 0 | 0 | 3 |  |
| 18 | 6.5 | 7.3 | click 1.29 | 966+26752 | 154 | 106 | 409 | 4 | 15 KB image/jpeg 374x325 |
| 19 | 17.9 | 19.1 | set_plan 0.00, click 0.90 | 424+27648 | 312 | 157 | 693 | 4 |  |
| 20 | 21.3 | 21.7 | check_done 0.01 | 546+28032 | 53 | 0 | 0 | 4 |  |
| 21 | 24.1 | 25.0 | mcp__gui_get_window_state 0.00 | 303+28544 | 126 | 47 | 224 | 4 |  |
| 22 | 4.0 | 4.6 | get_window_state 0.65 | 195+28800 | 80 | 0 | 0 | 4 |  |
| 23 | 2.9 | 3.4 | click 0.90 | 459+28928 | 48 | 0 | 0 | 5 | 15 KB image/jpeg 374x325 |
| 24 | 4.4 | 5.3 |  | 218+29312 | 114 | 0 | 0 | 5 |  |

model total 252.5 s, tool total 27.0 s over 24 turns; gate: 19 actions exec 12.9 s, gate decision 3.99 s, 15 observations 5.3 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow18-libreoffice_calc-535364ea-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.2 | list_windows 0.01, set_plan 0.01 | 454+4992 | 126 | 14 | 60 | 0 |  |
| 2 | 5.0 | 5.0 | get_window_state 0.51 | 459+5376 | 82 | 0 | 0 | 0 |  |
| 3 | 15.6 | 20.9 | run_actions 2.31 | 5569+5760 | 912 | 787 | 3284 | 1 | 182 KB image/jpeg 1396x823 |
| 4 | 24.8 | 25.5 | get_window_state 0.24 | 4044+11264 | 138 | 39 | 166 | 1 |  |
| 5 | 19.6 | 21.6 | get_window_state 0.23 | 555+15232 | 238 | 136 | 604 | 1 |  |
| 6 | 5.1 | 6.2 | run_actions 1.94 | 1030+15744 | 284 | 157 | 704 | 2 | 178 KB image/jpeg 1396x823 |
| 7 | 35.7 | 37.0 | run_actions 1.79 | 1131+16768 | 144 | 21 | 82 | 3 | 19 KB image/jpeg 316x361 |
| 8 | 43.8 | 45.4 | run_actions 2.19 | 1242+17792 | 219 | 111 | 442 | 4 | 178 KB image/jpeg 1396x823 |
| 9 | 56.0 | 59.6 | run_actions 0.95 | 1220+18944 | 629 | 524 | 1935 | 5 | 178 KB image/jpeg 1396x823 |
| 10 | 65.4 | 65.9 | run_actions 0.03 | 3770+20096 | 113 | 11 | 40 | 6 | 149 KB image/jpeg 1396x823 |
| 11 | 72.9 | 74.2 | run_actions 2.85 | 300+23808 | 331 | 106 | 483 | 6 |  |
| 12 | 67.4 | 68.1 | run_actions 2.74 | 1280+24064 | 144 | 21 | 75 | 6 |  |
| 13 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 431.8 s, tool total 15.8 s over 13 turns; gate: 20 actions exec 10.2 s, gate decision 0.10 s, 10 observations 4.4 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow18-libreoffice_calc-535364ea-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.1 | 2.8 | set_plan 0.00, list_windows 0.02 | 70+5376 | 140 | 0 | 0 | 0 |  |
| 2 | 1.4 | 1.8 | get_window_state 0.28 | 474+5376 | 108 | 8 | 37 | 0 |  |
| 3 | 1.9 | 5.9 | click 0.69 | 4899+5760 | 641 | 593 | 2526 | 0 |  |
| 4 | 1.6 | 2.2 | get_window_state 0.19 | 851+10624 | 136 | 19 | 78 | 0 |  |
| 5 | 1.8 | 2.3 | press_key 2.75 | 692+11392 | 115 | 55 | 239 | 0 |  |
| 6 | 2.0 | 2.6 | run_actions 1.65 | 343+12032 | 119 | 0 | 0 | 0 |  |
| 7 | 1.8 | 2.1 | click 1.74 | 2766+12288 | 48 | 0 | 0 | 0 |  |
| 8 | 1.7 | 1.9 | click 1.06 | 751+14976 | 72 | 24 | 101 | 0 |  |
| 9 | 2.6 | 3.4 | get_window_state 0.20 | 279+15616 | 170 | 55 | 237 | 0 |  |
| 10 | 2.3 | 3.1 | get_window_state 0.22 | 351+15872 | 169 | 52 | 215 | 0 |  |
| 11 | 2.4 | 4.1 | run_command 0.25 | 621+16128 | 292 | 176 | 731 | 0 |  |
| 12 | 2.6 | 6.8 | run_actions 3.97 | 619+16640 | 733 | 569 | 2448 | 0 |  |
| 13 | 2.9 | 5.0 | run_actions 5.22 | 3867+17152 | 433 | 229 | 872 | 0 |  |
| 14 | 3.0 | 3.9 | click 1.15 | 4489+20992 | 160 | 111 | 424 | 0 |  |
| 15 | 2.6 | 3.0 | click 2.55 | 560+25472 | 61 | 12 | 57 | 0 |  |
| 16 | 3.8 | 6.4 | click 1.88 | 1471+25984 | 442 | 393 | 1700 | 0 |  |
| 17 | 2.2 | 2.9 | get_window_state 0.08 | 23977+4096 | 152 | 51 | 214 | 0 |  |
| 18 | 7.8 | 11.0 | run_command 0.02 | 1082+27392 | 527 | 478 | 1711 | 1 | 23 KB image/jpeg 655x403 |
| 19 | 7.9 | 8.0 | run_command 0.02 | 653+28416 | 123 | 49 | 214 | 1 |  |
| 20 | 2.4 | 2.9 | run_command 0.02 | 230+29056 | 86 | 0 | 0 | 1 |  |
| 21 | 7.3 | 7.7 | run_command 0.02 | 298+29184 | 74 | 0 | 0 | 1 |  |
| 22 | 19.0 | 21.9 | run_command 1.92 | 217+29440 | 636 | 485 | 1707 | 1 |  |
| 23 | 9.2 | 9.8 | get_window_state 0.37 | 787+29568 | 119 | 0 | 0 | 1 |  |
| 24 | 9.0 | 11.9 | run_command 1.91 | 1736+30336 | 322 | 171 | 585 | 2 | 23 KB image/jpeg 655x403 |
| 25 | 9.1 | 9.5 | get_window_state 0.09 | 456+32000 | 101 | 0 | 0 | 2 |  |
| 26 | 11.6 | 12.6 | click 2.04 | 420+32384 | 159 | 111 | 474 | 3 | 24 KB image/jpeg 655x403 |
| 27 | 10.8 | 12.2 | get_window_state 0.08 | 1563+32768 | 262 | 161 | 740 | 3 |  |
| 28 | 5.6 | 6.6 | run_actions 1.36 | 536+34304 | 189 | 87 | 375 | 4 | 24 KB image/jpeg 655x403 |
| 29 | 3.2 | 3.2 | get_window_state 0.08 | 409+34816 | 101 | 0 | 0 | 4 |  |
| 30 | 12.4 | 18.3 |  | 373+35200 | 962 | 835 | 3531 | 5 | 24 KB image/jpeg 655x403 |

model total 195.8 s, tool total 31.8 s over 30 turns; gate: 26 actions exec 20.9 s, gate decision 2.98 s, 21 observations 6.8 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow19-multi_apps-81c425f5-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.1 | 3.7 | set_plan 0.00, list_windows 0.01 | 1221+4096 | 135 | 0 | 0 | 0 |  |
| 2 | 1.4 | 1.8 | get_window_state 0.39 | 729+4992 | 94 | 12 | 49 | 0 |  |
| 3 | 3.7 | 7.0 | run_actions 1.65 | 5830+5248 | 646 | 455 | 1726 | 1 | 132 KB image/jpeg 1396x823 |
| 4 | 16.8 | 18.2 | run_actions 0.55 | 926+11008 | 342 | 157 | 649 | 1 |  |
| 5 | 23.6 | 23.7 | get_window_state 0.19 | 498+11904 | 101 | 0 | 0 | 1 |  |
| 6 | 22.1 | 22.9 | run_actions 2.45 | 964+12288 | 170 | 22 | 84 | 2 | 133 KB image/jpeg 1396x823 |
| 7 | 31.2 | 32.2 | run_actions 1.65 | 1612+13184 | 221 | 64 | 229 | 3 | 134 KB image/jpeg 1396x823 |
| 8 | 37.7 | 38.1 | click 0.68 | 1206+14720 | 126 | 29 | 123 | 4 | 135 KB image/jpeg 1396x823 |
| 9 | 30.2 | 30.7 | get_window_state 0.31 | 291+15872 | 101 | 0 | 0 | 4 |  |
| 10 | 43.1 | 43.4 | click 0.98 | 5542+16128 | 73 | 8 | 26 | 4 |  |
| 11 | 44.8 | 45.3 | list_windows 0.01 | 237+21632 | 46 | 0 | 0 | 4 |  |
| 12 | 33.2 | 34.2 | get_window_state 0.19 | 221+21760 | 190 | 89 | 412 | 4 |  |
| 13 | 80.9 | 81.0 | run_actions 0.04 | 1085+21888 | 145 | 27 | 96 | 5 | 124 KB image/jpeg 1396x823 |
| 14 | 35.6 | 36.9 | run_actions 1.75 | 335+22912 | 279 | 134 | 588 | 5 |  |
| 15 | 43.4 | 43.6 | press_key 0.11 | 2892+23168 | 79 | 0 | 0 | 5 |  |
| 16 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 462.6 s, tool total 11.0 s over 16 turns; gate: 12 actions exec 6.9 s, gate decision 0.05 s, 11 observations 3.4 s (6 with screenshot), reviewer calls 0 (0.0 s)

## ow19-multi_apps-81c425f5-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.7 | 3.2 | set_plan 0.00, list_windows 0.01 | 1221+4096 | 125 | 0 | 0 | 0 |  |
| 2 | 1.3 | 2.0 | get_window_state 0.44 | 450+5248 | 95 | 13 | 47 | 0 |  |
| 3 | 15.6 | 20.9 | run_actions 4.12 | 5424+5632 | 868 | 700 | 2815 | 1 | 132 KB image/jpeg 1396x823 |
| 4 | 27.6 | 29.2 | run_actions 4.27 | 2358+11008 | 319 | 143 | 501 | 2 | 134 KB image/jpeg 1396x823 |
| 5 | 33.0 | 34.4 | press_key 1.07 | 1493+13312 | 266 | 168 | 689 | 3 | 134 KB image/jpeg 1396x823 |
| 6 | 4.7 | 5.5 | click 1.13 | 524+14720 | 177 | 112 | 520 | 3 |  |
| 7 | 2.5 | 2.9 | click 2.19 | 2727+15232 | 65 | 0 | 0 | 3 |  |
| 8 | 9.8 | 10.3 | list_windows 0.01 | 4176+17920 | 117 | 71 | 301 | 3 |  |
| 9 | 4.3 | 5.5 | run_actions 2.11 | 301+22016 | 237 | 92 | 366 | 3 |  |
| 10 | 3.5 | 3.9 | get_window_state 0.40 | 406+22272 | 82 | 0 | 0 | 3 |  |
| 11 | 42.8 | 43.5 | run_actions 2.65 | 3379+22656 | 161 | 32 | 139 | 4 | 55 KB image/jpeg 1396x823 |
| 12 | 44.9 | 45.6 | run_actions 2.72 | 997+25984 | 158 | 33 | 133 | 5 | 19 KB image/jpeg 350x282 |
| 13 | 45.8 | 46.3 | get_window_state 0.01 | 459+26880 | 82 | 0 | 0 | 5 |  |
| 14 | 49.0 | 49.5 | list_windows 0.01 | 193+27264 | 74 | 28 | 125 | 5 |  |
| 15 | 54.3 | 54.4 | get_window_state 0.45 | 243+27392 | 140 | 40 | 194 | 5 |  |
| 16 | 45.9 | 48.9 | run_actions 2.50 | 4522+27520 | 546 | 398 | 1727 | 6 | 78 KB image/jpeg 1396x823 |
| 17 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 406.1 s, tool total 24.1 s over 17 turns; gate: 18 actions exec 14.1 s, gate decision 2.02 s, 15 observations 6.6 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow20-libreoffice_calc-0326d92d-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.3 | 3.2 | set_plan 0.00, list_windows 0.01 | 1396+4096 | 145 | 0 | 0 | 0 |  |
| 2 | 1.8 | 2.2 | get_window_state 0.50 | 531+5376 | 89 | 7 | 30 | 0 |  |
| 3 | 12.4 | 20.4 | run_actions 14.40 | 4592+5888 | 1597 | 1126 | 4086 | 1 | 136 KB image/jpeg 1396x823 |
| 4 | 2.5 | 6.4 | run_command 0.13 | 3551+10368 | 697 | 607 | 2596 | 1 |  |
| 5 | 16.8 | 18.7 | run_command 0.10 | 884+13824 | 401 | 275 | 1077 | 1 |  |
| 6 | 5.3 | 8.8 | run_command 0.00 | 850+14592 | 959 | 332 | 1009 | 1 |  |
| 7 | 12.8 | 15.0 | run_command 0.15 | 1058+15360 | 557 | 0 | 0 | 1 |  |
| 8 | 5.0 | 6.1 | run_actions 0.02 | 896+16384 | 205 | 84 | 333 | 1 |  |
| 9 | 12.3 | 13.3 | get_window_state 0.29 | 334+17280 | 204 | 88 | 340 | 1 |  |
| 10 | 17.5 | 17.8 | run_actions 1.58 | 669+17536 | 122 | 0 | 0 | 1 |  |
| 11 | 19.7 | 20.4 | run_actions 0.03 | 2652+18176 | 164 | 26 | 106 | 1 |  |
| 12 | 15.6 | 17.0 | get_window_state 0.23 | 385+20736 | 247 | 148 | 594 | 1 |  |
| 13 | 2.7 | 3.1 | click 0.02 | 483+21120 | 67 | 0 | 0 | 1 |  |
| 14 | 2.3 | 3.1 | click 0.01 | 271+21504 | 153 | 76 | 295 | 1 |  |
| 15 | 14.6 | 14.9 | click 0.75 | 234+21760 | 94 | 0 | 0 | 1 |  |
| 16 | 2.0 | 2.4 | get_window_state 0.31 | 293+21888 | 100 | 0 | 0 | 1 |  |
| 17 | 28.1 | 28.7 | get_window_state 0.32 | 2055+22144 | 103 | 21 | 74 | 2 | 140 KB image/jpeg 1396x823 |
| 18 | 17.4 | 17.8 | click 0.90 | 326+24192 | 84 | 0 | 0 | 2 |  |
| 19 | 5.9 | 6.3 | get_window_state 0.34 | 247+24448 | 100 | 0 | 0 | 2 |  |
| 20 | 52.3 | 58.0 | run_command 0.14 | 2238+24576 | 948 | 708 | 2829 | 3 | 137 KB image/jpeg 1396x823 |
| 21 | 45.4 | 45.8 | set_plan 0.00 | 1243+26752 | 61 | 0 | 0 | 3 |  |
| 22 | 48.4 | 50.6 | check_done 0.14 | 246+27904 | 53 | 0 | 0 | 3 |  |
| 23 | 3.3 | 4.4 |  | 1095+28032 | 179 | 52 | 243 | 3 |  |

model total 384.2 s, tool total 20.4 s over 23 turns; gate: 25 actions exec 16.9 s, gate decision 0.09 s, 9 observations 2.5 s (3 with screenshot), reviewer calls 0 (0.0 s)

## ow20-libreoffice_calc-0326d92d-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.8 | 2.7 | list_windows 0.01, set_plan 0.01 | 116+5376 | 150 | 0 | 0 | 0 |  |
| 2 | 2.6 | 3.2 | get_window_state 0.29 | 542+5376 | 107 | 7 | 30 | 0 |  |
| 3 | 3.7 | 3.8 | run_command 0.11 | 3910+5888 | 186 | 87 | 315 | 0 |  |
| 4 | 2.1 | 3.8 | click 0.71 | 578+9728 | 290 | 225 | 756 | 0 |  |
| 5 | 2.2 | 13.8 | run_command 0.13 | 2460+10240 | 2079 | 1425 | 6028 | 0 |  |
| 6 | 3.6 | 4.4 | click 1.66 | 2440+12672 | 103 | 38 | 140 | 0 |  |
| 7 | 2.2 | 2.5 | click 2.06 | 2531+15104 | 65 | 0 | 0 | 0 |  |
| 8 | 2.3 | 3.4 | get_window_state 0.21 | 3036+17536 | 175 | 74 | 311 | 0 |  |
| 9 | 10.2 | 11.0 | press_key 1.33 | 1016+20480 | 179 | 81 | 310 | 1 | 136 KB image/jpeg 1396x823 |
| 10 | 16.1 | 16.2 | list_windows 0.01 | 811+21376 | 68 | 22 | 100 | 1 |  |
| 11 | 7.5 | 8.4 | run_command 0.12 | 165+22144 | 183 | 36 | 158 | 1 |  |
| 12 | 3.8 | 3.9 | run_actions 0.54 | 400+22272 | 146 | 32 | 159 | 1 |  |
| 13 | 2.9 | 3.1 | get_window_state 0.00 | 285+22656 | 101 | 0 | 0 | 1 |  |
| 14 | 16.4 | 16.8 | get_window_state 0.21 | 215+22912 | 129 | 12 | 53 | 1 |  |
| 15 | 3.9 | 4.8 | scroll 3.21 | 965+23040 | 169 | 43 | 185 | 2 | 136 KB image/jpeg 1396x823 |
| 16 | 33.2 | 33.8 | check_done 0.13, set_plan 0.13 | 1227+23936 | 209 | 105 | 401 | 3 | 120 KB image/jpeg 1396x823 |
| 17 | 41.3 | 41.7 |  | 709+25088 | 74 | 0 | 0 | 3 |  |

model total 177.2 s, tool total 10.9 s over 17 turns; gate: 9 actions exec 6.2 s, gate decision 0.51 s, 12 observations 3.5 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow21-gimp-734d6579-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.1 | 3.2 | list_apps 0.02, list_windows 0.02 | 314+4992 | 57 | 0 | 0 | 0 |  |
| 2 | 2.6 | 4.1 | set_plan 0.00, get_window_state 0.36 | 1913+5248 | 228 | 46 | 187 | 0 |  |
| 3 | 11.1 | 11.5 | click 0.31 | 1597+7040 | 53 | 5 | 25 | 1 | 39 KB image/jpeg 647x606 |
| 4 | 3.1 | 3.5 | get_window_state 0.45 | 181+8576 | 100 | 18 | 59 | 1 |  |
| 5 | 11.2 | 14.1 | get_window_state 0.38 | 4781+8704 | 463 | 348 | 1516 | 2 | 82 KB image/jpeg 1003x681 |
| 6 | 19.0 | 19.1 | get_window_state 0.51 | 580+13440 | 137 | 39 | 163 | 2 |  |
| 7 | 11.7 | 14.6 | click 1.48 | 1028+13952 | 413 | 365 | 1645 | 2 |  |
| 8 | 10.4 | 10.8 | get_window_state 0.10 | 509+14976 | 83 | 0 | 0 | 2 |  |
| 9 | 22.3 | 24.3 | click 1.14 | 682+15360 | 178 | 53 | 216 | 3 | 81 KB image/jpeg 1003x681 |
| 10 | 5.1 | 5.2 | list_windows 0.01 | 313+16000 | 45 | 0 | 0 | 3 |  |
| 11 | 4.1 | 4.3 | get_window_state 0.53 | 214+16256 | 80 | 0 | 0 | 3 |  |
| 12 | 6.7 | 7.4 | run_actions 5.20 | 531+16384 | 192 | 17 | 63 | 4 | 27 KB image/jpeg 580x432 |
| 13 | 24.7 | 25.5 | run_actions 0.00 | 2952+16896 | 145 | 23 | 50 | 4 |  |
| 14 | 20.2 | 20.8 | click 0.49 | 229+19840 | 94 | 0 | 0 | 4 |  |
| 15 | 20.6 | 20.7 | click 0.75 | 659+19968 | 48 | 0 | 0 | 5 | 28 KB image/jpeg 580x432 |
| 16 | 28.5 | 28.9 | click 0.87 | 165+20608 | 73 | 13 | 54 | 5 |  |
| 17 | 23.1 | 23.6 | get_window_state 0.62 | 220+20736 | 80 | 0 | 0 | 5 |  |
| 18 | 2.7 | 2.9 | click 0.02 | 819+20864 | 48 | 0 | 0 | 6 | 17 KB image/jpeg 250x591 |
| 19 | 2.8 | 4.8 | run_actions 1.39 | 206+21632 | 362 | 241 | 884 | 6 |  |
| 20 | 31.7 | 32.0 | get_window_state 0.10 | 2971+21760 | 105 | 22 | 100 | 6 |  |
| 21 | 35.9 | 37.5 | click 0.64 | 608+24704 | 316 | 253 | 970 | 7 | 86 KB image/jpeg 1003x681 |
| 22 | 30.8 | 31.3 | get_window_state 0.61 | 522+25216 | 80 | 0 | 0 | 7 |  |
| 23 | 5.8 | 6.0 | run_actions 1.41 | 805+25728 | 195 | 73 | 295 | 8 | 18 KB image/jpeg 288x517 |
| 24 | 34.4 | 34.7 | run_actions 0.58, run_command 0.60 | 2765+26496 | 280 | 76 | 323 | 8 |  |
| 25 | 42.0 | 43.2 | run_command 5.84 | 518+29184 | 126 | 37 | 153 | 8 |  |
| 26 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 433.9 s, tool total 24.4 s over 26 turns; gate: 18 actions exec 17.1 s, gate decision 0.06 s, 15 observations 4.9 s (8 with screenshot), reviewer calls 0 (0.0 s)

## ow21-gimp-734d6579-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.4 | 2.7 | list_apps 0.02, run_command 0.04 | 1210+4096 | 103 | 31 | 134 | 0 |  |
| 2 | 1.8 | 2.5 | set_plan 0.00, list_windows 0.01 | 1955+5248 | 140 | 12 | 42 | 0 |  |
| 3 | 2.7 | 2.8 | get_window_state 0.36 | 348+7168 | 105 | 23 | 73 | 0 |  |
| 4 | 1.9 | 10.1 | run_command 0.03, click 1.51 | 4911+7424 | 1180 | 1049 | 4089 | 1 | 87 KB image/jpeg 1003x681 |
| 5 | 4.7 | 6.8 | run_command 0.04, press_key 2.10 | 4857+12288 | 388 | 203 | 811 | 1 |  |
| 6 | 13.6 | 17.3 | run_actions 3.01 | 3156+17024 | 633 | 516 | 1911 | 1 |  |
| 7 | 7.2 | 7.9 | get_window_state 0.10 | 3274+20096 | 123 | 40 | 185 | 1 |  |
| 8 | 3.1 | 4.0 | get_window_state 0.07 | 738+23296 | 167 | 67 | 281 | 2 | 87 KB image/jpeg 1003x681 |
| 9 | 4.8 | 4.9 | click 1.03 | 1195+23936 | 65 | 0 | 0 | 2 |  |
| 10 | 18.0 | 18.5 | click 1.28 | 210+25088 | 114 | 20 | 86 | 2 |  |
| 11 | 8.6 | 10.8 | press_key 0.64 | 4375+25216 | 493 | 433 | 1722 | 3 | 84 KB image/jpeg 1003x681 |
| 12 | 33.9 | 34.6 | press_key 2.37 | 3430+29568 | 101 | 41 | 182 | 4 | 81 KB image/jpeg 1003x681 |
| 13 | 39.0 | 39.4 | type_text 1.06 | 669+32896 | 82 | 0 | 0 | 5 | 2 KB image/jpeg 168x34 |
| 14 | 34.4 | 34.8 | get_window_state 0.13 | 178+33536 | 82 | 0 | 0 | 5 |  |
| 15 | 42.1 | 42.8 | press_key 0.26 | 708+33664 | 79 | 0 | 0 | 6 | 5 KB image/jpeg 168x261 |
| 16 | 41.5 | 41.7 | list_windows 0.01 | 215+34304 | 45 | 0 | 0 | 6 |  |
| 17 | 3.7 | 4.3 | get_window_state 0.11 | 214+34432 | 119 | 36 | 152 | 6 |  |
| 18 | 49.8 | 51.6 | run_actions 0.02 | 679+34560 | 303 | 179 | 746 | 7 | 81 KB image/jpeg 1003x681 |
| 19 | 3.8 | 4.7 | run_actions 1.23 | 427+35200 | 155 | 25 | 97 | 7 |  |
| 20 | 8.7 | 9.1 | type_text 1.03 | 541+35584 | 82 | 0 | 0 | 7 |  |
| 21 | 5.4 | 5.6 | get_window_state 0.11 | 178+36096 | 82 | 0 | 0 | 7 |  |
| 22 | 44.5 | 44.7 | click 1.83 | 708+36224 | 100 | 34 | 122 | 8 | 5 KB image/jpeg 168x261 |
| 23 | 45.4 | 45.6 | list_windows 0.01 | 339+36864 | 45 | 0 | 0 | 8 |  |
| 24 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 447.3 s, tool total 18.4 s over 24 turns; gate: 16 actions exec 7.7 s, gate decision 3.80 s, 17 observations 4.2 s (8 with screenshot), reviewer calls 0 (0.0 s)

## ow22-libreoffice_calc-4de54231-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.7 | 2.3 | set_plan 0.00, list_apps 0.02 | 510+4992 | 137 | 0 | 0 | 0 |  |
| 2 | 1.7 | 1.9 | list_windows 0.01 | 1878+5376 | 59 | 13 | 51 | 0 |  |
| 3 | 2.1 | 2.1 | get_window_state 0.33 | 202+7168 | 64 | 0 | 0 | 0 |  |
| 4 | 8.4 | 10.2 | run_command 0.13 | 4868+7296 | 254 | 119 | 462 | 1 | 137 KB image/jpeg 1396x823 |
| 5 | 15.6 | 20.4 | run_actions 0.50 | 1226+12160 | 878 | 609 | 1783 | 1 |  |
| 6 | 3.2 | 3.3 | click 0.52 | 1193+13312 | 143 | 49 | 224 | 1 |  |
| 7 | 4.8 | 5.6 | run_actions 5.34 | 1104+14464 | 212 | 0 | 0 | 2 | 136 KB image/jpeg 1396x823 |
| 8 | 28.4 | 29.2 | run_actions 0.48 | 1824+15488 | 181 | 20 | 64 | 2 |  |
| 9 | 4.4 | 5.4 | get_window_state 0.18 | 343+17280 | 157 | 74 | 312 | 2 |  |
| 10 | 43.8 | 43.9 | run_actions 3.22 | 1046+17536 | 161 | 0 | 0 | 3 | 139 KB image/jpeg 1396x823 |
| 11 | 29.9 | 32.7 | run_actions 3.27 | 3231+18560 | 557 | 301 | 1084 | 4 | 167 KB image/jpeg 1396x823 |
| 12 | 45.3 | 49.3 | run_actions 7.10 | 3809+21760 | 785 | 399 | 1211 | 4 |  |
| 13 | 5.9 | 7.0 | run_actions 0.72 | 3837+25472 | 222 | 93 | 355 | 4 |  |
| 14 | 62.8 | 63.4 | run_command 0.15 | 27483+4096 | 198 | 29 | 149 | 4 |  |
| 15 | 4.4 | 5.8 | run_command 0.12 | 2924+29184 | 212 | 60 | 216 | 4 |  |
| 16 | 56.1 | 56.5 | check_done 0.10, set_plan 0.10 | 396+32000 | 100 | 0 | 0 | 4 |  |
| 17 | 30.5 | 31.1 |  | 1500+32384 | 107 | 25 | 116 | 4 |  |

model total 370.0 s, tool total 22.3 s over 17 turns; gate: 36 actions exec 17.4 s, gate decision 0.10 s, 11 observations 3.9 s (5 with screenshot), reviewer calls 0 (0.0 s)

## ow22-libreoffice_calc-4de54231-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.2 | 2.9 | set_plan 0.02, list_apps 0.02 | 126+5376 | 134 | 0 | 0 | 0 |  |
| 2 | 4.8 | 5.1 | list_windows 0.01 | 440+5504 | 59 | 13 | 48 | 0 |  |
| 3 | 2.0 | 2.5 | get_window_state 0.42 | 172+5888 | 100 | 0 | 0 | 0 |  |
| 4 | 18.4 | 21.2 | run_actions 3.22 | 4821+6016 | 696 | 527 | 1513 | 1 | 136 KB image/jpeg 1396x823 |
| 5 | 18.1 | 19.0 | run_actions 3.88 | 3896+10752 | 185 | 10 | 28 | 1 |  |
| 6 | 15.9 | 18.0 | run_actions 4.85 | 2269+14592 | 467 | 282 | 915 | 1 |  |
| 7 | 17.2 | 19.0 | type_text 2.04 | 1295+16768 | 366 | 211 | 654 | 1 |  |
| 8 | 14.6 | 15.0 | press_key 1.28 | 448+18048 | 79 | 0 | 0 | 1 |  |
| 9 | 17.4 | 19.8 | run_actions 4.74 | 1303+18432 | 438 | 247 | 995 | 1 |  |
| 10 | 10.4 | 13.1 | run_command 0.11 | 1528+19712 | 283 | 145 | 558 | 1 |  |
| 11 | 11.6 | 12.7 | run_command 0.30 | 1338+21120 | 289 | 101 | 453 | 1 |  |
| 12 | 16.8 | 18.0 | run_command 0.02 | 888+22400 | 193 | 129 | 511 | 1 |  |
| 13 | 9.3 | 9.5 | list_windows 0.01 | 432+23168 | 156 | 111 | 449 | 1 |  |
| 14 | 6.4 | 7.3 | check_done 0.08, set_plan 0.08 | 260+23552 | 220 | 62 | 213 | 1 |  |
| 15 | 16.0 | 16.5 |  | 1445+23808 | 80 | 0 | 0 | 1 |  |

model total 199.3 s, tool total 21.1 s over 15 turns; gate: 23 actions exec 18.3 s, gate decision 0.07 s, 8 observations 2.2 s (1 with screenshot), reviewer calls 0 (0.0 s)

## ow23-libreoffice_impress-455d3c66-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 3.4 | 3.6 | set_plan 0.00 | 3+5376 | 107 | 0 | 0 | 0 |  |
| 2 | 3.8 | 4.1 | list_windows 0.01 | 176+5376 | 28 | 0 | 0 | 0 |  |
| 3 | 4.7 | 5.5 | get_window_state 0.21 | 269+5504 | 161 | 79 | 358 | 0 |  |
| 4 | 1.7 | 2.0 | click 0.30 | 2522+5760 | 76 | 11 | 41 | 1 | 113 KB image/jpeg 1396x823 |
| 5 | 2.1 | 2.7 | get_window_state 0.00 | 236+8192 | 102 | 0 | 0 | 1 |  |
| 6 | 12.1 | 12.3 | get_window_state 0.41 | 297+8320 | 80 | 0 | 0 | 1 |  |
| 7 | 23.8 | 24.2 | click 0.77 | 1235+8576 | 65 | 0 | 0 | 2 | 114 KB image/jpeg 1396x823 |
| 8 | 9.8 | 10.2 | get_window_state 0.21 | 243+9728 | 64 | 0 | 0 | 2 |  |
| 9 | 14.4 | 14.8 | click 0.88 | 1592+9856 | 108 | 43 | 175 | 3 | 31 KB image/jpeg 782x470 |
| 10 | 27.2 | 27.7 | get_window_state 0.22 | 278+11392 | 64 | 0 | 0 | 3 |  |
| 11 | 28.3 | 28.6 | click 1.38 | 999+11648 | 65 | 0 | 0 | 4 | 28 KB image/jpeg 571x324 |
| 12 | 31.0 | 31.1 | click 0.83 | 305+12544 | 158 | 93 | 410 | 4 |  |
| 13 | 30.0 | 31.1 | press_key 0.85 | 344+12800 | 264 | 187 | 693 | 4 |  |
| 14 | 22.6 | 22.8 | get_window_state 0.26 | 442+13056 | 84 | 20 | 74 | 4 |  |
| 15 | 5.0 | 5.6 | press_key 0.84 | 1623+13440 | 133 | 56 | 217 | 5 | 45 KB image/jpeg 782x470 |
| 16 | 2.8 | 3.1 | press_key 0.88 | 310+14976 | 77 | 0 | 0 | 5 |  |
| 17 | 3.2 | 3.9 | press_key 0.10 | 221+15232 | 77 | 0 | 0 | 5 |  |
| 18 | 5.5 | 6.3 | run_actions 2.97 | 260+15360 | 165 | 21 | 88 | 5 |  |
| 19 | 4.7 | 5.6 | press_key 0.88 | 553+15616 | 109 | 30 | 126 | 5 |  |
| 20 | 11.1 | 11.3 | type_text 0.89 | 240+16128 | 60 | 0 | 0 | 5 |  |
| 21 | 8.7 | 8.8 | get_window_state 0.32 | 263+16256 | 98 | 0 | 0 | 5 |  |
| 22 | 12.0 | 12.4 | click 0.77 | 605+16512 | 86 | 0 | 0 | 6 | 31 KB image/jpeg 782x470 |
| 23 | 6.1 | 6.5 | get_window_state 0.16 | 291+17024 | 76 | 12 | 53 | 6 |  |
| 24 | 7.7 | 8.1 | click 0.84 | 1567+17280 | 65 | 0 | 0 | 7 | 20 KB image/jpeg 408x497 |
| 25 | 32.3 | 32.6 | run_command 0.04 | 196+18816 | 61 | 0 | 0 | 7 |  |
| 26 | 12.4 | 12.9 | set_plan 0.00 | 253+18944 | 109 | 0 | 0 | 7 |  |
| 27 | 30.4 | 30.6 | check_done 0.13 | 304+19072 | 52 | 0 | 0 | 7 |  |
| 28 | 34.8 | 35.2 |  | 351+19328 | 57 | 0 | 0 | 7 |  |

model total 403.6 s, tool total 15.2 s over 28 turns; gate: 16 actions exec 12.5 s, gate decision 0.07 s, 9 observations 1.6 s (7 with screenshot), reviewer calls 0 (0.0 s)

## ow23-libreoffice_impress-455d3c66-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.5 | 3.0 | set_plan 0.00, list_windows 0.01 | 3+5376 | 125 | 0 | 0 | 0 |  |
| 2 | 4.7 | 4.8 | run_command 0.02, get_window_state 0.28 | 379+5376 | 181 | 31 | 128 | 0 |  |
| 3 | 10.5 | 11.2 | click 0.60 | 4055+5632 | 112 | 64 | 275 | 1 | 113 KB image/jpeg 1396x823 |
| 4 | 10.1 | 10.2 | click 1.76 | 2713+9600 | 55 | 7 | 23 | 1 |  |
| 5 | 2.3 | 3.0 | get_window_state 0.09 | 1250+12288 | 152 | 52 | 221 | 1 |  |
| 6 | 3.2 | 3.6 | click 1.38 | 887+13440 | 181 | 87 | 346 | 2 | 31 KB image/jpeg 782x470 |
| 7 | 4.7 | 5.0 | click 2.82 | 1741+14208 | 110 | 16 | 56 | 3 | 44 KB image/jpeg 782x470 |
| 8 | 2.4 | 3.3 | click 1.12 | 641+15872 | 175 | 81 | 295 | 4 | 32 KB image/jpeg 782x470 |
| 9 | 17.6 | 18.3 | click 1.55 | 592+16512 | 120 | 26 | 85 | 5 | 44 KB image/jpeg 782x470 |
| 10 | 11.5 | 11.7 | click 1.07 | 597+17024 | 463 | 369 | 1450 | 6 | 30 KB image/jpeg 782x470 |
| 11 | 25.1 | 25.3 | press_key 1.25 | 957+17536 | 113 | 34 | 123 | 7 | 46 KB image/jpeg 782x470 |
| 12 | 25.0 | 25.6 | press_key 0.61 | 549+18432 | 96 | 17 | 72 | 8 | 46 KB image/jpeg 782x470 |
| 13 | 23.4 | 24.2 | run_actions 4.35 | 566+18944 | 185 | 0 | 0 | 9 | 31 KB image/jpeg 782x470 |
| 14 | 22.9 | 23.5 | click 1.00 | 1668+19456 | 89 | 40 | 172 | 9 |  |
| 15 | 14.5 | 17.1 | run_command 0.04 | 186+21120 | 65 | 0 | 0 | 9 |  |
| 16 | 18.9 | 19.9 | check_done 0.09, set_plan 0.09 | 351+21248 | 203 | 56 | 236 | 9 |  |
| 17 | 18.5 | 18.6 |  | 18059+4096 | 42 | 0 | 0 | 9 |  |

model total 228.4 s, tool total 18.1 s over 17 turns; gate: 16 actions exec 11.3 s, gate decision 2.23 s, 13 observations 3.5 s (9 with screenshot), reviewer calls 0 (0.0 s)

## ow24-libreoffice_calc-1de60575-direct-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.0 | 4.0 | set_plan 0.01, list_windows 0.01 | 462+4992 | 152 | 0 | 0 | 0 |  |
| 2 | 1.7 | 2.7 | get_window_state 0.46 | 501+5376 | 107 | 7 | 30 | 0 |  |
| 3 | 14.2 | 14.9 | run_command 1.27 | 7695+4992 | 231 | 78 | 333 | 1 | 198 KB image/jpeg 1396x823 |
| 4 | 21.2 | 23.5 | run_actions 2.25 | 505+12672 | 590 | 447 | 1734 | 1 |  |
| 5 | 18.5 | 19.1 | click 0.02 | 3390+13056 | 128 | 44 | 167 | 1 |  |
| 6 | 17.5 | 18.6 | run_actions 0.52 | 10918+5760 | 191 | 56 | 214 | 1 |  |
| 7 | 22.9 | 23.3 | click 0.29 | 2989+16384 | 96 | 0 | 0 | 1 |  |
| 8 | 21.6 | 22.6 | get_window_state 0.35 | 278+19328 | 100 | 0 | 0 | 1 |  |
| 9 | 35.8 | 36.1 | get_window_state 0.21 | 4986+19584 | 104 | 22 | 76 | 2 | 180 KB image/jpeg 1396x823 |
| 10 | 38.2 | 38.6 | click 0.74 | 775+24448 | 84 | 0 | 0 | 3 | 7 KB image/jpeg 282x173 |
| 11 | 35.0 | 35.4 | get_window_state 0.18 | 200+25216 | 100 | 0 | 0 | 3 |  |
| 12 | 34.9 | 36.3 | run_command 0.04 | 1713+25344 | 417 | 327 | 1449 | 4 | 22 KB image/jpeg 655x403 |
| 13 | 38.4 | 41.6 | get_window_state 0.05 | 596+27008 | 595 | 478 | 2127 | 4 |  |
| 14 | 43.8 | 50.4 | run_command 0.22 | 816+27520 | 1314 | 1089 | 4568 | 4 |  |
| 15 | 45.8 | 45.9 | run_command 0.06 | 1471+28288 | 210 | 127 | 521 | 4 |  |
| 16 | 34.1 | 36.3 | run_command 0.89 | 395+29696 | 573 | 184 | 665 | 4 |  |
| 17 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 449.4 s, tool total 7.6 s over 17 turns; gate: 12 actions exec 5.4 s, gate decision 0.05 s, 8 observations 1.7 s (4 with screenshot), reviewer calls 0 (0.0 s)

## ow24-libreoffice_calc-1de60575-gate8-s0

| turn | ttft s | model s | tools (s) | input tok (uncached+cached) | out tok | reasoning tok | think chars | images in context | new images |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 2.9 | 3.6 | list_windows 0.01, set_plan 0.01 | 78+5376 | 137 | 0 | 0 | 0 |  |
| 2 | 1.4 | 1.7 | get_window_state 0.39 | 473+5376 | 71 | 7 | 31 | 0 |  |
| 3 | 12.2 | 13.8 | run_command 0.25 | 6863+5760 | 319 | 196 | 716 | 1 | 198 KB image/jpeg 1396x823 |
| 4 | 13.2 | 17.0 | run_command 0.21 | 1376+12544 | 752 | 577 | 2389 | 1 |  |
| 5 | 19.5 | 20.4 | click 0.73 | 973+13824 | 169 | 121 | 480 | 1 |  |
| 6 | 19.7 | 20.4 | get_window_state 0.39 | 425+14720 | 104 | 23 | 91 | 1 |  |
| 7 | 28.1 | 29.0 | run_actions 6.34 | 987+15104 | 325 | 185 | 803 | 2 | 198 KB image/jpeg 1396x823 |
| 8 | 45.1 | 45.7 | run_actions 0.02 | 4155+16000 | 98 | 19 | 65 | 3 | 183 KB image/jpeg 1396x823 |
| 9 | 34.5 | 34.9 | click 1.94 | 283+20096 | 72 | 24 | 85 | 3 |  |
| 10 | 27.2 | 27.4 | get_window_state 0.22 | 299+20352 | 121 | 20 | 92 | 3 |  |
| 11 | 30.9 | 31.8 | click 1.33 | 913+20608 | 362 | 314 | 1277 | 4 | 183 KB image/jpeg 1396x823 |
| 12 | 4.1 | 4.2 | click 1.75 | 715+21504 | 60 | 12 | 60 | 4 |  |
| 13 | 3.2 | 8.6 | run_command 0.04, get_window_state 0.10 | 1497+22144 | 831 | 707 | 3010 | 4 |  |
| 14 | 2.7 | 4.7 | run_command 0.02 | 1233+23552 | 339 | 255 | 1026 | 5 | 22 KB image/jpeg 655x403 |
| 15 | 46.6 | 50.2 | get_window_state 0.09 | 511+24704 | 562 | 445 | 1691 | 5 |  |
| 16 | 57.7 | 59.7 | click 0.00 | 826+25088 | 368 | 238 | 1045 | 5 |  |
| 17 | 57.0 | 58.8 | run_command 0.04 | 550+25856 | 313 | 231 | 1034 | 5 |  |
| 18 | 26.3 | 30.0 | run_command 0.02 | 463+26368 | 887 | 832 | 3440 | 5 |  |
| 19 | 0.0 | 0.0 |  | 0+0 |  |  |  |  |  |

model total 461.9 s, tool total 13.9 s over 19 turns; gate: 14 actions exec 8.0 s, gate decision 1.55 s, 11 observations 3.7 s (5 with screenshot), reviewer calls 0 (0.0 s)

