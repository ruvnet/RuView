"""
Faithful Python port of the window/baseline/metric definitions in
v2/crates/ruforecast/crates/ruforecast-train/examples/bidmc_prepare.rs and
v2/crates/ruforecast/crates/ruforecast-core/src/{baseline,metrics}.rs so the
zero-shot pretrained-model comparison is apples-to-apples with RuForecast's
own real-data evaluation.

CONTEXT=64, HORIZON=12, VARIATES=3 (HR, RESP, SpO2), quantiles as in
ForecastModelConfig::tiny_ci(), seasonal_period=12 (the `evaluate` CLI default).
"""
import csv
import math
import os

CONTEXT = 64
HORIZON = 12
WINDOW_ROWS = CONTEXT + HORIZON
VARIATE_NAMES = ["HR", "RESP", "SpO2"]
QUANTILES = [0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95]
SEASONAL_PERIOD = 12
RAW_DIR = "/tmp/bidmc-raw"


def read_patient(pid: int):
    path = os.path.join(RAW_DIR, f"bidmc_{pid:02d}_Numerics.csv")
    if not os.path.exists(path):
        return None
    rows = []
    with open(path, newline="") as f:
        reader = csv.reader(f)
        next(reader)  # header
        for parts in reader:
            if len(parts) < 5:
                continue
            parts = [p.strip() for p in parts]
            try:
                hr = float(parts[1])
                resp = float(parts[3])
                spo2 = float(parts[4])
            except ValueError:
                return None  # malformed row -> treat like the Rust `?` early-return
            rows.append((hr, resp, spo2))
    return rows


def build_window(pid: int):
    """One window per patient: first CONTEXT rows are context, next HORIZON
    rows are targets. Mirrors build_window() in bidmc_prepare.rs exactly
    (context_start = row 0, no scanning for a "best" window)."""
    rows = read_patient(pid)
    if rows is None or len(rows) < WINDOW_ROWS:
        return None
    first = rows[:WINDOW_ROWS]
    for row in first:
        for v in row:
            if not math.isfinite(v):
                return None
    context = first[:CONTEXT]          # list of (hr, resp, spo2), CONTEXT rows
    targets = first[CONTEXT:WINDOW_ROWS]  # HORIZON rows
    return {"patient": pid, "context": context, "targets": targets}


def judge_split(judge: str):
    if judge == "a":
        train_ids = list(range(1, 38))
        test_ids = list(range(38, 54))
    elif judge == "b":
        train_ids = [i for i in range(1, 54) if i % 2 == 1]
        test_ids = [i for i in range(1, 54) if i % 2 == 0]
    else:
        raise ValueError(judge)
    return train_ids, test_ids


def build_test_windows(judge: str):
    _, test_ids = judge_split(judge)
    windows = []
    excluded = []
    for pid in test_ids:
        w = build_window(pid)
        if w is None:
            excluded.append(pid)
        else:
            windows.append(w)
    return windows, excluded


# ---- baselines (mirror ruforecast-core::baseline.rs) ----

def last_value_forecast(context):
    """Repeat the last observed context row for every future step (every
    quantile gets the same point value -> degenerate distribution)."""
    last = context[-1]
    return [[last[v] for _ in QUANTILES] for _ in range(HORIZON) for v in range(len(last))]
    # NOTE: shape handled explicitly in metric functions below instead; see
    # last_value_predictions() for the actually-used [horizon][variate] form.


def last_value_predictions(context):
    last = context[-1]
    return [[last[v] for v in range(len(last))] for _ in range(HORIZON)]


def seasonal_naive_predictions(context, period=SEASONAL_PERIOD):
    """context has CONTEXT=64 rows; base = len(context) - period; for step in
    0..HORIZON: row = base + step % period."""
    base = len(context) - period
    preds = []
    for step in range(HORIZON):
        row = base + (step % period)
        preds.append(list(context[row]))
    return preds


# ---- metrics (mirror ruforecast-core::metrics.rs) ----

def mae(actual_flat, predicted_flat):
    total, count = 0.0, 0
    for a, p in zip(actual_flat, predicted_flat):
        total += abs(a - p)
        count += 1
    return total / count if count else float("nan")


def weighted_quantile_loss(actual_flat, quantile_preds_flat):
    """actual_flat: list of HORIZON*VARIATES actuals in step-major order
    (step0-var0,step0-var1,step0-var2, step1-var0, ...).
    quantile_preds_flat: same order, each entry is a list over QUANTILES.
    Matches weighted_quantile_loss() in metrics.rs: sum pinball over every
    (cell, quantile), normalize by 2 / (num_quantiles * sum(|actual|))."""
    loss = 0.0
    scale = 0.0
    for actual, preds in zip(actual_flat, quantile_preds_flat):
        scale += abs(actual)
        for q, pred in zip(QUANTILES, preds):
            residual = actual - pred
            loss += q * residual if residual >= 0 else (q - 1.0) * residual
    if scale == 0.0:
        raise ValueError("weighted quantile loss undefined: all-zero actual targets")
    return 2.0 * loss / (len(QUANTILES) * scale)


def flatten_targets_step_major(targets):
    """targets: HORIZON rows of VARIATES values -> flat step-major list."""
    out = []
    for row in targets:
        out.extend(row)
    return out


def point_to_quantile_preds(point_preds):
    """point_preds: HORIZON rows of VARIATES point values (deterministic
    baseline) -> step-major flat list where every quantile equals the point
    value, matching how LastValue/SeasonalNaive fill every quantile slot
    with the same repeated value."""
    out = []
    for row in point_preds:
        for v in row:
            out.append([v for _ in QUANTILES])
    return out
