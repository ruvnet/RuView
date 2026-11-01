# NTU-Fi HumanID: comparison contract

The current short-window LSTM is an experimental baseline. Its results do not
establish state of the art or identity recognition from the local BFI hardware.
This note records sources checked on 2026-09-29 and the conditions needed for a
fair comparison.

## Resolve the split names before comparing results

**MEASURED archive inventory:** the public `NTU-Fi-HumanID.zip` contains
294 files in `train_amp` and 546 in `test_amp`, across 14 identities. Its SHA256
is `1fef4e66b088b94160b14f8b950538a5455f5a62514146e17e53d19f3732a094`.
Every file contains `CSIamp`, a real double array with shape `(342, 2000)`.
The private inventory, numeric validation, and per-file hashes reproduce these
observations. No exact duplicate arrays were found, before or after decimation;
that check does not establish independence of physical acquisition sessions.

The author code explicitly **trains on `test_amp` and evaluates on `train_amp`**
for HumanID. This explains the README's 546 training / 294 test counts.
Using the directory names literally produces a different experiment.
See the HumanID branch in the author's
[loader configuration](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark/blob/main/util.py)
and the [dataset description](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark#dataset).

The initial local protocol used literal directory names. Preserve its split and
receipts under an explicit archive-directory protocol label. A later author
protocol needs a separate manifest and run. Do not relabel a completed run or
swap its train and test assignments after inspecting results. Development on
both assignments also prevents treating their overlapping records as a fresh,
untouched holdout.

## Published reference and differences

**CLAIMED by SenseFi, supervised Table 3:**

| Model | HumanID accuracy | Parameters |
| --- | ---: | ---: |
| CNN-5 | 97.14% | 0.478 million |
| GRU | 98.96% | 0.079 million |
| LSTM | 97.19% | 0.105 million |
| BiLSTM | 99.38% | 0.210 million |

These are paper results, averaged over three seeds. They are not measurements
of this implementation. The paper's generic 80:20 split statement also differs
from its HumanID count table, so reproduce an explicitly identified code/data
version. [SenseFi paper, Tables 2–3 and implementation details](https://pmc.ncbi.nlm.nih.gov/articles/PMC10028433/).

The author loader takes every fourth time sample and presents one full
`(3, 114, 500)` recording to the model. Its LSTM has one layer with hidden width
64 and one prediction per recording. Our bounded LSTM uses shorter windows;
averaging their probabilities is a different model and evaluation procedure.
[Author loader](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark/blob/main/dataset.py),
[author model](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark/blob/main/NTU_Fi_model.py).

There is a normalization discrepancy to resolve as well: the paper describes
min-max normalization, while the released CSI loader uses fixed constants
`42.3199` and `4.9802`. Their fitting population is not established here. The
local trainer instead fits feature normalization exclusively on training rows.
Report that difference; do not adopt unexplained constants to chase a score.
[Paper](https://pmc.ncbi.nlm.nih.gov/articles/PMC10028433/),
[released preprocessing](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark/blob/main/dataset.py).

## Next comparisons

1. Freeze source hashes, split assignments, preprocessing, model, seed list and
   selection rule. Use whole training files for validation and score a frozen
   checkpoint on test only after selection. Keep normalization within training.
2. Report recording accuracy, balanced accuracy, confusion matrix, majority
   baseline, recording coverage and windows per recording. Label window metrics
   separately. Correlated windows are not independent trials. Physical session
   separation remains unknown; sample indices are not measured seconds.
3. Compare full-recording GRU and CNN-5/LeNet, then BiLSTM, under the same frozen
   local protocol. These are practical author baselines, with no guaranteed
   improvement. Exact reproduction must also match the author's model readout
   and training schedule. [SenseFi model implementations](https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark/blob/main/NTU_Fi_model.py).
4. Keep future CSI-to-BFI transfer experiments separate. Require labeled BFI
   recordings, independent acquisition sessions, and target-device evaluation
   before claiming that a public CSI classifier works on the local link.

Retain dataset attribution and the private provenance receipt. The
[dataset release](https://data.mendeley.com/datasets/dzvgyxkx2f/1) is CC BY 4.0;
the author repository's MIT code license is separate.
