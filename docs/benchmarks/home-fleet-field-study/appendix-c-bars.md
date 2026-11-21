# Appendix C. Pre-registered bars: timing proof and deviations

Proof types. "commit" means the bar text (or its hash) is in a version-control commit earlier than the first score of that test, which is independent of any file system clock. "seal file" means a hash or pre-registration file whose modification time is earlier than the first score; modification times can be altered, though nothing in the record shows they were. "claim" means the record only says "pre-registered" and no earlier artefact survives. For the weekly decision-day tests, "scoring" means the one scored run; what the bar guards there is the scoring step, because the underlying days already existed. Commit hashes are short hashes of the study fork.

| test | bar fixed | proof type | predates scoring? | deviations | verdict |
|---|---|---|---|---|---|
| Row-174 full-rate capture (L001) | 2026-09-25 commit 3b755824 | commit | yes (before capture) | post-hoc 2-h sensitivity; resident-mask discrepancy; start delayed | FAIL (outcome C) |
| R13 A vs B (L009) | 2026-10-05 commit ecf0d93a | commit | yes (vs result commit) | R14 mask: days < 50 % kept not counted | FAIL (both) |
| R14/R15 features (L010) | 2026-10-06 commit 8236e1c0 (mask), bar = R13 | commit | yes | none | FAIL |
| R16 NN candidate (L074) | 2026-10-06 commit 504b3839; addenda 10-09 | commit + NAS sha | yes | train-before mode added; labels regenerated before scoring | FAIL, holds no |
| R17 steering features (L013) | 2026-10-06 NAS file | seal file (git hash after result) | claimed only | none | FAIL |
| R18 FG vs LS (L011) | 2026-10-06 commit 0d5dc157 | commit | yes | none | feature of record FG; R13 bar FAIL |
| R19 vs live label (L012) | 2026-10-06 (addenda to end of day) NAS file | seal file | claimed only | C3 added as candidate before its scores | PASS vs live label |
| R22 witness rule, bar days (L016) | 2026-10-07 stated; file rewritten 10-09 | claim | no (original text gone) | R22b redesign after reading result | FAIL |
| R22b decision days | 2026-10-09 NAS file | seal file | yes (before 10-11 scoring) | reproduction gate mismatch | VOID (rule terms fail) |
| R23 hold, bar days (L015) | 2026-10-07 stated; file rewritten 10-09 | claim | no (original text gone) | R23b derived from result; S live before decision days | FAIL |
| R23b decision days | 2026-10-09 NAS file | seal file | yes | same | VOID (FP term fails on 2 of 3) |
| R9 presence_v2 week (L019) | 2026-10-09 commit fe43943e | commit | yes | none to bar | FAIL |
| R24 legacy frames (L018) | 2026-10-10 commit 541a0cc8 | commit | yes | run slipped 10-15 -> 10-16 (runner bug), addenda 1-2 | FAIL |
| Home/away forward test (L020) | 2026-10-19 commit 721e5770 | commit + seal | yes | truth files gained lines; 5 of 14 days unlabelled | H1 PASS, H2 PASS |
| H3 room activity (L035) | 2026-10-20 commit b7f6e366 | commit + seal | yes | catch bars demoted before seal (option C) | rooms 0, 3 PASS; 4, 7 FAIL; catches nil |
| Node-traffic prereg (L047/L154) | 2026-10-18 commit 66a8b547 | commit + seal | yes | pilot ~11 h late; fleet push never ran | FAIL (pilot bar) |
| Node-traffic addendum 1 | 2026-10-20 commit ed8bda0c | commit + seal | yes | pre-treatment change of gate | not exercised |
| Blind week S3-S5 (L038/L039) | 2026-10-29 commit 8664e743 | commit + seal | yes | week dates earlier than strategy doc; outage left 70.5 of 168 h | NOT PROVEN |
| Positions bypass re-score | 2026-11-20 NAS file | file mtime (12 s before score) | claimed only | post-hoc confusion check | bar FAIL (lift 0.197 both) |
| Raw-CSI probe (L075) | 2026-11-20 commit 59e1f359 | commit | yes | one hour skipped; 3 days dropped | INVALID-DESIGN |
| Full-set screens | 2026-10-22 commits | commit order (same evening) | yes (minutes) | design replaced by screens; rxobs control not clean | 0 survivors in 3 families |
| Still-person endpoint (L062) | 2026-10-09 commit 4ed93a97 | commit | yes | docstring time differs from commit time | FAIL, FAIL, FAIL |
| Desk study (L064) | 2026-10-13 NAS file | seal file (hash in git at result) | claimed only | in-text times later than file mtimes | FAIL |
| Row-184 AGC gates (L100/L101) | 2026-09-28 commit 82953ee3; retest 63c0dfb1 | commit | yes | D1-D3 before numbers; 5 of 9 nodes in first Q | 2(a) PASS; 2(b) KILL; re-test KILL |
