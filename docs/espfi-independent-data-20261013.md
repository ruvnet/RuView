# Independent CSI evaluation data audit

Checked 2026-10-13. **No signal recordings were downloaded, ingested or evaluated.** Anonymous requests retrieved repository source, dataset catalog metadata, a README, a label map and the initial metadata CSV header only. No credentials, upstream messages or paid resources were used.

The old ESP-Fi test was opened during the first pilot and the development partition has been reused adaptively. See [label and confirmation audit](espfi-label-audit-20261013.md). Another public dataset can supply independent evidence only after permissions, label compatibility and a frozen evaluation protocol are established.

## Decision matrix

| Candidate | Version and anonymous access | Data permission | Immediate decision |
| --- | --- | --- | --- |
| CSI-Bench | Kaggle version 12, updated 2026-06-16T23:36:11.083Z. Catalog, file listing and individual README/label-map downloads returned HTTP 200 without login | Dataset explicitly CC BY-NC-ND 4.0 and research only | Technically accessible metadata, but not admitted to the commercial model pipeline |
| WiFiTAD | Code revision `0bf3098bbc990cc5713415811ca08c05e10f6522`, committed 2025-08-26. Dataset linked as one Drive ZIP, without an explicit dataset version | Repository code Apache 2.0; a distinct dataset permission statement was not verified | Useful continuous evaluation candidate, pending dataset terms and verified archive access |

## CSI-Bench: strongest grouping, restrictive data terms

[Author repository](https://github.com/guozhen-jenn-zhu/CSI-Bench-Real-WiFi-Sensing-Benchmark/blob/b1beac7b0fa46e01748901195ecfbe309dfc0180/README.md), inspected revision `b1beac7b0fa46e01748901195ecfbe309dfc0180`, explicitly identifies Kaggle version 12 as the corrected benchmark release. Its last commit timestamp was 2026-06-21T22:45:59Z. Use this release rather than earlier reshaped arrays or third party mirrors.

The [author dataset](https://www.kaggle.com/datasets/guozhenjennzhu/csi-bench/versions/12) catalog reports 79,258,851,301 bytes. Anonymous [catalog](https://www.kaggle.com/api/v1/datasets/list?search=csi-bench) and [versioned file listing](https://www.kaggle.com/api/v1/datasets/list/guozhenjennzhu/csi-bench?datasetVersionNumber=12) were accessible. This is evidence for metadata access, not verification of every large data download.

The [versioned dataset README](https://www.kaggle.com/api/v1/datasets/download/guozhenjennzhu/csi-bench/README.md?datasetVersionNumber=12) was retrieved successfully: 6,033 bytes. It identifies processed HDF5, raw MATLAB files, and a `RawContinuousRecording` directory containing amplitude and phase. It explicitly restricts the dataset to research purposes and CC BY-NC-ND 4.0. These terms must not be replaced by the repository's MIT badge. Moreover, the repository README itself declares CC BY-NC-ND while its root LICENSE is MIT, so even the code permission presentation is inconsistent.

The [HAR label map](https://www.kaggle.com/api/v1/datasets/download/guozhenjennzhu/csi-bench/Multitask%2FHumanActivityRecognition%2Fmetadata%2Flabel_mapping.json?datasetVersionNumber=12), retrieved successfully at 284 bytes, defines five classes: jumping, running, seated-breathing, walking and wavinghand. They do not match all seven ESP-Fi classes. Semantic intersections cannot yet be frozen while the ESP-Fi naming conflict remains unresolved.

The versioned HAR `sample_metadata.csv` returned HTTP 200 with Content-Length 11,501,693 bytes; only its initial 4,096 bytes were read, to verify the header: `id,file_path,subject,user,activity,distance,num_sub,c_freq,environment,device,session,is_multi_subject,label`. No complete CSV counts or signal files were inspected. The repository describes user, activity, environment, device and session grouping, metadata CSVs, explicit label maps, and separate cross user/environment/device tests. Shared Multitask HDF5 recordings back HAR, identity and proximity tasks; those tasks cannot be treated as independent datasets. The author [project page](https://ai-iot-sensing.github.io/projects/project.html) reports 35 users, 26 environments and 16 device types across the whole collection; these are not verified counts for a proposed HAR slice.

Retrieved metadata SHA-256: dataset README `5835d7bdcddd94c7c3e4ac839dcca35d6928488950aaa090a9249eba1fa65067`; HAR label map `b5f649bbfc3940e056d51d1905504bdc8a7afcf603dd84a34f96951793de16e7`. These hashes attest metadata, not the uninspected signal archive.

Smallest next admissible operation: metadata inspection only. Read the complete versioned HAR metadata and official split lists, then determine whole session groups and class coverage without reading signal payloads. Do not choose sessions using candidate performance. If the intended use is permitted, predeclare a bounded pilot using whole sessions and a supported label contract before fetching just its referenced HDF5 files. Exact pilot bytes and the number of independently grouped sessions remain unknown. An isolated parser smoke test would not constitute confirmation accuracy.

## WiFiTAD: continuous annotations, limited environmental coverage

The [author paper](https://arxiv.org/html/2412.16233v2), revision dated 2025-01-26, and [AAAI publication](https://ojs.aaai.org/index.php/AAAI/article/view/32035), published 2025-04-11, describe 553 continuous recordings and 2,114 annotated activity instances. Collection used Intel 5300 hardware, 30 subcarriers at 100 Hz, three participants and one office. Labels include walking, running, jumping, waving, falling, sitting and standing with temporal boundaries. The published 70:30 partition is not a demonstrated unseen room split.

The [pinned loader](https://github.com/AVC2-UESTC/WiFiTAD/blob/0bf3098bbc990cc5713415811ca08c05e10f6522/TAD/dataset/load_csi.py) expects per recording NPY arrays, CSV recording and annotation tables, and a class index text file. The public code is Apache 2.0. That does not establish permission for the separately hosted dataset.

The official [Drive archive](https://drive.google.com/file/d/1gy0ppFtypVTtgBfrFzdMJUbXTb1MbPSK/view) rendered only a sign in link through the available web reader. This result does not establish that the file requires login for every download method, but anonymous signal access was not verified. No archive checksum, byte size, distinct dataset license or participant identifier schema was verified. No alternative mirror was substituted.

After these gaps are resolved, the smallest useful slice would be a complete annotated recording for parser and boundary alignment tests, followed by an independently grouped set with all required classes for scoring. One recording or one room cannot establish generalization across rooms. Do not concatenate ESP-Fi trials to imitate this continuous dataset.

## Recommendation and acceptance

Prioritize CSI-Bench version 12 for metadata planning because its group and split contracts are explicit. Do not train or evaluate the commercial candidate on its recordings under an assumption of permissive rights. WiFiTAD is the stronger match for continuous event timing, but first needs verified dataset permission and access. Neither is currently an admitted fresh confirmation set. Restricted WiFi-Wild is not proposed as an accessible fallback.

Acceptance: a frozen intake manifest records exact version, permitted purpose, source hashes, labels and whole session group membership; its adapter passes schema and timing checks; baseline and candidate are frozen before a previously unseen confirmation split is scored once. Any schema tuning data must be excluded from that confirmation split.
