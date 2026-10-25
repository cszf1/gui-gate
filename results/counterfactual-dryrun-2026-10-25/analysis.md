# Counterfactual dry run (fixture twin, scripted controller - NOT model data)

# Counterfactual branches (step success)

branches 1260, checkpoints 60, families ['loading', 'mixed', 'popup', 'relabel'], unfaithful restores 0

| treatment | checkpoints | delta vs direct | 95% CI (family bootstrap) | neg / zero / pos | mean cost s | tokens | person s |
|---|---|---|---|---|---|---|---|
| ask | 60 | +0.550 | +0.372..+0.728 | 11/11/38 | 20.3 | 0 | 20.0 |
| check | 60 | +0.233 | +0.100..+0.383 | 0/46/14 | 4.6 | 175 | 0.0 |
| extra_reason | 60 | -0.039 | -0.161..+0.133 | 15/38/7 | 7.3 | 400 | 0.0 |
| look_full | 60 | +0.617 | +0.467..+0.767 | 0/23/37 | 9.3 | 1400 | 0.0 |
| look_light | 60 | +0.300 | +0.117..+0.500 | 0/42/18 | 7.3 | 300 | 0.0 |
| matched_wait | 60 | +0.083 | +0.000..+0.267 | 0/55/5 | 7.3 | 0 | 0.0 |

look_light minus matched_wait (information beyond waiting): +0.217 [+0.117, +0.317]

# Counterfactual branches (task success)

branches 1260, checkpoints 60, families ['loading', 'mixed', 'popup', 'relabel'], unfaithful restores 0

| treatment | checkpoints | delta vs direct | 95% CI (family bootstrap) | neg / zero / pos | mean cost s | tokens | person s |
|---|---|---|---|---|---|---|---|
| ask | 60 | -0.128 | -0.233..-0.044 | 9/51/0 | 20.3 | 0 | 20.0 |
| check | 60 | -0.117 | -0.217..-0.033 | 7/53/0 | 4.6 | 175 | 0.0 |
| extra_reason | 60 | -0.122 | -0.267..-0.006 | 10/49/1 | 7.3 | 400 | 0.0 |
| look_full | 60 | -0.050 | -0.150..+0.033 | 5/53/2 | 9.3 | 1400 | 0.0 |
| look_light | 60 | -0.167 | -0.300..-0.067 | 11/48/1 | 7.3 | 300 | 0.0 |
| matched_wait | 60 | -0.083 | -0.233..+0.033 | 6/53/1 | 7.3 | 0 | 0.0 |

look_light minus matched_wait (information beyond waiting): -0.083 [-0.183, +0.017]

## Test families only, with the train-family predictor

# Counterfactual branches (step success)

branches 630, checkpoints 30, families ['loading', 'popup'], unfaithful restores 0

| treatment | checkpoints | delta vs direct | 95% CI (family bootstrap) | neg / zero / pos | mean cost s | tokens | person s |
|---|---|---|---|---|---|---|---|
| ask | 30 | +0.567 | +0.278..+0.844 | 7/3/20 | 20.3 | 0 | 20.0 |
| check | 30 | +0.200 | +0.000..+0.433 | 0/24/6 | 4.5 | 170 | 0.0 |
| extra_reason | 30 | +0.011 | -0.211..+0.311 | 8/17/5 | 7.3 | 400 | 0.0 |
| look_full | 30 | +0.633 | +0.400..+0.867 | 0/11/19 | 9.3 | 1400 | 0.0 |
| look_light | 30 | +0.300 | +0.000..+0.667 | 0/21/9 | 7.3 | 300 | 0.0 |
| matched_wait | 30 | +0.167 | +0.000..+0.433 | 0/25/5 | 7.3 | 0 | 0.0 |

look_light minus matched_wait (information beyond waiting): +0.133 [+0.033, +0.267]

## Incremental reliability (equal-mass bins of predicted increment; observed = paired, family-bootstrap 95% CI)

| treatment | bin | n | families | mean predicted | mean observed | 95% CI |
|---|---|---|---|---|---|---|
| look_light | 0 | 10 | 2 | +0.167 | +0.200 | +0.000..+0.600 |
| look_light | 1 | 10 | 2 | +0.300 | +0.300 | +0.000..+0.800 |
| look_light | 2 | 5 | 2 | +0.375 | +0.400 | +0.000..+0.800 |
| look_light | 3 | 5 | 2 | +1.000 | +0.400 | +0.125..+1.000 |
| extra_reason | 0 | 10 | 2 | -0.389 | -0.167 | -0.467..+0.233 |
| extra_reason | 1 | 10 | 2 | -0.000 | +0.167 | -0.133..+0.600 |
| extra_reason | 2 | 5 | 2 | +0.000 | +0.133 | +0.000..+0.400 |
| extra_reason | 3 | 5 | 2 | +0.167 | -0.067 | -0.200..+0.000 |
| matched_wait | 0 | 30 | 2 | +0.000 | +0.167 | +0.000..+0.433 |
| look_full | 0 | 10 | 2 | +0.167 | +0.200 | +0.000..+0.600 |
| look_full | 1 | 10 | 2 | +0.500 | +0.800 | +0.500..+1.000 |
| look_full | 2 | 5 | 2 | +0.875 | +1.000 | +1.000..+1.000 |
| look_full | 3 | 5 | 2 | +1.000 | +0.800 | +0.400..+1.000 |
| check | 0 | 10 | 2 | +0.167 | +0.200 | +0.000..+0.600 |
| check | 1 | 5 | 2 | +0.250 | +0.200 | +0.000..+0.600 |
| check | 2 | 10 | 2 | +0.300 | +0.200 | +0.000..+0.600 |
| check | 3 | 5 | 2 | +1.000 | +0.200 | +0.000..+0.600 |
| ask | 0 | 10 | 2 | +0.056 | -0.067 | -0.300..+0.300 |
| ask | 1 | 10 | 2 | +0.433 | +0.900 | +0.600..+1.000 |
| ask | 2 | 5 | 2 | +0.792 | +1.000 | +1.000..+1.000 |
| ask | 3 | 5 | 2 | +1.000 | +0.733 | +0.200..+1.000 |

## Unseen layouts (evaluation only)

# Counterfactual branches (step success)

branches 3780, checkpoints 180, families ['layout:animals-mixed-n8-2032', 'layout:animals-relabel-n8-2026', 'layout:cities-loading-n8-2033', 'layout:cities-popup-n8-2027', 'layout:colors-relabel-n8-2028', 'layout:colors-relabel-n8-2034', 'layout:instruments-loading-n8-2029', 'layout:instruments-relabel-n8-2035', 'layout:planets-mixed-n8-2030', 'layout:planets-relabel-n8-2036', 'layout:tools-mixed-n8-2031', 'layout:tools-popup-n8-2037'], unfaithful restores 0

| treatment | checkpoints | delta vs direct | 95% CI (family bootstrap) | neg / zero / pos | mean cost s | tokens | person s |
|---|---|---|---|---|---|---|---|
| ask | 180 | +0.504 | +0.389..+0.607 | 19/58/103 | 20.3 | 0 | 20.0 |
| check | 180 | +0.217 | +0.128..+0.300 | 0/141/39 | 4.4 | 167 | 0.0 |
| extra_reason | 180 | -0.089 | -0.167..-0.009 | 51/112/17 | 7.3 | 400 | 0.0 |
| look_full | 180 | +0.556 | +0.461..+0.644 | 0/80/100 | 9.3 | 1400 | 0.0 |
| look_light | 180 | +0.250 | +0.139..+0.356 | 0/135/45 | 7.3 | 300 | 0.0 |
| matched_wait | 180 | +0.050 | +0.000..+0.133 | 0/171/9 | 7.3 | 0 | 0.0 |

look_light minus matched_wait (information beyond waiting): +0.200 [+0.144, +0.261]

## Incremental reliability (equal-mass bins of predicted increment; observed = paired, family-bootstrap 95% CI)

| treatment | bin | n | families | mean predicted | mean observed | 95% CI |
|---|---|---|---|---|---|---|
| check | 0 | 60 | 12 | +0.122 | +0.083 | +0.000..+0.200 |
| check | 1 | 42 | 12 | +0.250 | +0.286 | +0.128..+0.444 |
| check | 2 | 60 | 12 | +0.300 | +0.283 | +0.133..+0.433 |
| check | 3 | 18 | 12 | +1.000 | +0.278 | +0.083..+0.556 |
| ask | 0 | 44 | 12 | +0.056 | +0.023 | -0.121..+0.190 |
| ask | 1 | 60 | 12 | +0.433 | +0.578 | +0.367..+0.756 |
| ask | 2 | 16 | 8 | +0.750 | +0.646 | +0.356..+0.929 |
| ask | 3 | 42 | 12 | +0.792 | +0.833 | +0.711..+0.939 |
| ask | 4 | 18 | 12 | +1.000 | +0.537 | +0.222..+0.778 |
| look_full | 0 | 44 | 12 | +0.167 | +0.114 | +0.000..+0.267 |
| look_full | 1 | 60 | 12 | +0.500 | +0.617 | +0.450..+0.767 |
| look_full | 2 | 16 | 8 | +0.750 | +0.688 | +0.429..+0.929 |
| look_full | 3 | 42 | 12 | +0.875 | +0.857 | +0.718..+0.972 |
| look_full | 4 | 18 | 12 | +1.000 | +0.611 | +0.333..+0.833 |
| extra_reason | 0 | 44 | 12 | -0.389 | -0.303 | -0.483..-0.085 |
| extra_reason | 1 | 76 | 12 | -0.035 | -0.039 | -0.127..+0.055 |
| extra_reason | 2 | 42 | 12 | +0.000 | +0.008 | -0.052..+0.060 |
| extra_reason | 3 | 18 | 12 | +0.167 | -0.000 | -0.056..+0.056 |
| matched_wait | 0 | 180 | 12 | +0.000 | +0.050 | +0.000..+0.128 |
| look_light | 0 | 60 | 12 | +0.122 | +0.083 | +0.000..+0.200 |
| look_light | 1 | 60 | 12 | +0.300 | +0.317 | +0.150..+0.483 |
| look_light | 2 | 42 | 12 | +0.375 | +0.333 | +0.178..+0.487 |
| look_light | 3 | 18 | 12 | +1.000 | +0.389 | +0.167..+0.750 |
