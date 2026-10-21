| format | bytes/image mean (KB) | click hit (1 img) | text exact (1 img) | text case-insens. | all (1 img) | all (3 img) | TTFT 1 img mean / p50 s | total 1 img mean s | TTFT 3 img mean / p50 s | total 3 img mean s | prompt tok/image | failed attempts (429/503, retried) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| png | 326.3 | 25/88 (28%) | 38/58 (66%) | 38/58 (66%) | 63/146 (43%) | 20/50 (40%) | 21.1 / 19.8 | 23.6 | 66.4 / 66.0 | 70.3 | 773.0 | 33 |
| jpg80 | 123.8 | 20/88 (23%) | 41/58 (71%) | 41/58 (71%) | 61/146 (42%) | 26/50 (52%) | 8.0 / 6.8 | 10.2 | 24.7 / 27.4 | 27.8 | 773.0 | 40 |
| jpg60 | 92.9 | 22/88 (25%) | 43/58 (74%) | 43/58 (74%) | 65/146 (45%) | 19/50 (38%) | 6.7 / 5.7 | 10.0 | 18.8 / 17.9 | 21.7 | 773.0 | 28 |
| jpg80half | 40.4 | 9/88 (10%) | 27/58 (47%) | 27/58 (47%) | 36/146 (25%) | 7/50 (14%) | 4.2 / 3.0 | 7.9 | 7.8 / 7.0 | 10.7 | 252.0 | 30 |
| webp80 | 54.2 | 18/88 (20%) | 37/58 (64%) | 37/58 (64%) | 55/146 (38%) | 18/50 (36%) | 4.6 / 3.6 | 6.9 | 12.5 / 13.0 | 16.2 | 773.0 | 40 |

Cold calls only (provider reported 0 cached prompt tokens, i.e. this image was not served from the prefix cache):
| format | n (1 img) | TTFT 1 img mean / p50 s | total 1 img mean s | n (3 img) | TTFT 3 img mean / p50 s | total 3 img mean s | output tok mean (1 img, mostly reasoning) |
|---|---|---|---|---|---|---|---|
| png | 70 | 21.4 / 19.7 | 23.9 | 24 | 64.8 / 64.4 | 68.7 | 404.5 |
| jpg80 | 74 | 8.5 / 7.0 | 10.7 | 21 | 25.3 / 28.5 | 28.6 | 387.7 |
| jpg60 | 73 | 7.0 / 6.2 | 11.2 | 24 | 17.2 / 13.7 | 19.9 | 535.5 |
| jpg80half | 43 | 3.9 / 3.2 | 6.2 | 9 | 6.7 / 7.6 | 9.1 | 585.0 |
| webp80 | 73 | 4.9 / 3.8 | 7.1 | 23 | 11.4 / 9.6 | 14.5 | 355.9 |

Paired vs PNG (same question, same image count, same repeat): format right & PNG wrong / format wrong & PNG right
- jpg80: n=196, +15 / -11
- jpg60: n=196, +18 / -17
- jpg80half: n=196, +7 / -47
- webp80: n=196, +11 / -21

Accuracy by app (1 image, all repeats):
| format | Chromium-browser | Gimp | libreoffice-calc | libreoffice-impress | libreoffice-writer |
|---|---|---|---|---|---|
| png | 5/8 | 12/18 | 20/50 | 9/30 | 17/40 |
| jpg80 | 6/8 | 12/18 | 18/50 | 10/30 | 15/40 |
| jpg60 | 6/8 | 10/18 | 22/50 | 10/30 | 17/40 |
| jpg80half | 5/8 | 5/18 | 10/50 | 6/30 | 10/40 |
| webp80 | 6/8 | 10/18 | 18/50 | 10/30 | 11/40 |
