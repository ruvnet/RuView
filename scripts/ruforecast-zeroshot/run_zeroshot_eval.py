"""
Real zero-shot pretrained-checkpoint comparison for RuForecast (see
docs/benchmarks/ruforecast.md). Runs Amazon Chronos-Bolt (and TimesFM, if
installed) zero-shot inference against the SAME BIDMC test windows,
baselines, and metrics RuForecast's own `evaluate` CLI uses, and prints a
markdown table.

No fine-tuning. No training. Every "model" forecast below comes from a
public pretrained checkpoint's zero-shot .predict()/.forecast() call.
"""
import sys
import time
import numpy as np

sys.path.insert(0, "/tmp")  # bidmc_windows.py copied there
from bidmc_windows import (
    CONTEXT, HORIZON, VARIATE_NAMES, QUANTILES, SEASONAL_PERIOD,
    build_test_windows, judge_split,
    last_value_predictions, seasonal_naive_predictions,
    mae, weighted_quantile_loss, flatten_targets_step_major, point_to_quantile_preds,
)


def eval_baselines(windows):
    """Returns dict: name -> (mae, wql) aggregated over ALL test windows and
    variates, matching how the Rust evaluate CLI aggregates (one running
    mean/scale accumulation across the whole test set, not averaged per
    window)."""
    results = {}
    for name, fn in [("last-value", last_value_predictions),
                      ("seasonal-naive", lambda ctx: seasonal_naive_predictions(ctx, SEASONAL_PERIOD))]:
        all_actual, all_point_pred, all_qpred = [], [], []
        for w in windows:
            preds = fn(w["context"])  # HORIZON rows x VARIATES
            actual_flat = flatten_targets_step_major(w["targets"])
            point_flat = flatten_targets_step_major(preds)
            qpred_flat = point_to_quantile_preds(preds)
            all_actual.extend(actual_flat)
            all_point_pred.extend(point_flat)
            all_qpred.extend(qpred_flat)
        results[name] = (mae(all_actual, all_point_pred), weighted_quantile_loss(all_actual, all_qpred))
    return results


def chronos_zero_shot_predictions(windows, model_name="amazon/chronos-bolt-base", device="cuda"):
    """Per-variate univariate zero-shot forecast (Chronos has no native
    multivariate mode) via ChronosPipeline. Returns list parallel to
    `windows`, each entry: HORIZON rows x VARIATES quantile-lists."""
    from chronos import BaseChronosPipeline
    pipeline = BaseChronosPipeline.from_pretrained(model_name, device_map=device, torch_dtype="bfloat16" if device == "cuda" else "float32")
    import torch

    all_preds = []
    for w in windows:
        ctx = w["context"]  # HORIZON... actually CONTEXT rows x VARIATES
        per_variate_q = []
        for v in range(len(VARIATE_NAMES)):
            series = torch.tensor([row[v] for row in ctx], dtype=torch.float32)
            quantiles, _mean = pipeline.predict_quantiles(
                inputs=series,
                prediction_length=HORIZON,
                quantile_levels=QUANTILES,
            )
            # quantiles shape: [1, HORIZON, len(QUANTILES)]
            per_variate_q.append(quantiles[0].numpy())  # [HORIZON, Q]
        # reshape to HORIZON rows, each VARIATES entries of Q-length lists
        rows = []
        for h in range(HORIZON):
            row = [per_variate_q[v][h].tolist() for v in range(len(VARIATE_NAMES))]
            rows.append(row)
        all_preds.append(rows)
    return all_preds


def timesfm_zero_shot_predictions(windows, model_repo="google/timesfm-2.5-200m-pytorch"):
    """Per-variate univariate zero-shot forecast via TimesFM 2.5 200M torch.
    Quantile output has 10 columns; per the installed package's source
    (timesfm_2p5_base.py: `decode_index: int = 5`), column 5 is the
    point/mean forecast and columns 1..9 are quantiles [0.1..0.9] in order
    (verified empirically: columns 1-9 are monotonic and column 5 exactly
    equals the returned point forecast on every sampled window/variate;
    column 0 is an unused duplicate channel, not a real quantile level).
    TimesFM has no 0.05/0.95 levels (clamped to 0.1/0.9, same treatment
    Chronos-Bolt applies for out-of-range levels) and no native 0.25/0.75
    (linearly interpolated between the adjacent 0.1-spaced levels it does
    have)."""
    import timesfm
    import numpy as np

    m = timesfm.TimesFM_2p5_200M_torch.from_pretrained(model_repo)
    m.compile(timesfm.ForecastConfig(
        max_context=CONTEXT, max_horizon=HORIZON, normalize_inputs=True,
        use_continuous_quantile_head=True, fix_quantile_crossing=True,
    ))
    native_levels = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]  # columns 1..9

    def sample_at(row10, level):
        if level <= native_levels[0]:
            return row10[1]
        if level >= native_levels[-1]:
            return row10[9]
        for i in range(len(native_levels) - 1):
            lo, hi = native_levels[i], native_levels[i + 1]
            if lo <= level <= hi:
                col_lo, col_hi = 1 + i, 1 + i + 1
                if hi == lo:
                    return row10[col_lo]
                frac = (level - lo) / (hi - lo)
                return row10[col_lo] + frac * (row10[col_hi] - row10[col_lo])
        raise ValueError(level)

    all_preds = []
    for w in windows:
        ctx = w["context"]
        per_variate_rows = []
        for v in range(len(VARIATE_NAMES)):
            series = np.array([row[v] for row in ctx], dtype=np.float32)
            _point, quant = m.forecast(horizon=HORIZON, inputs=[series])
            per_variate_rows.append(quant[0])  # [HORIZON, 10]
        rows = []
        for h in range(HORIZON):
            row = []
            for v in range(len(VARIATE_NAMES)):
                row10 = per_variate_rows[v][h]
                row.append([sample_at(row10, q) for q in QUANTILES])
            rows.append(row)
        all_preds.append(rows)
    return all_preds


def eval_pretrained(name, predict_fn, windows):
    t0 = time.time()
    preds = predict_fn(windows)
    elapsed = time.time() - t0
    all_actual, all_point_pred, all_qpred = [], [], []
    median_idx = QUANTILES.index(0.50)
    for w, pred_rows in zip(windows, preds):
        actual_flat = flatten_targets_step_major(w["targets"])
        qpred_flat = []
        point_flat = []
        for row in pred_rows:  # HORIZON rows, each VARIATES quantile-lists
            for var_qs in row:
                qpred_flat.append(list(var_qs))
                point_flat.append(var_qs[median_idx])
        all_actual.extend(actual_flat)
        all_point_pred.extend(point_flat)
        all_qpred.extend(qpred_flat)
    m = mae(all_actual, all_point_pred)
    wql = weighted_quantile_loss(all_actual, all_qpred)
    return m, wql, elapsed


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", nargs="+", default=["chronos-bolt-base"])
    parser.add_argument("--judges", nargs="+", default=["a", "b"])
    args = parser.parse_args()

    print("| Judge | Model | Test windows | MAE | WQL | Wall time (s) |")
    print("|---|---|---:|---:|---:|---:|")

    for judge in args.judges:
        windows, excluded = build_test_windows(judge)
        train_ids, test_ids = judge_split(judge)
        print(f"<!-- judge {judge}: {len(windows)} test windows from {len(test_ids)} candidate patients, excluded={excluded} -->", file=sys.stderr)

        baselines = eval_baselines(windows)
        for name, (m, wql) in baselines.items():
            print(f"| {judge} | {name} (baseline) | {len(windows)} | {m:.4f} | {wql:.5f} | - |")

        for model in args.models:
            if model == "chronos-bolt-base":
                fn = lambda w: chronos_zero_shot_predictions(w, "amazon/chronos-bolt-base")
            elif model == "chronos-bolt-small":
                fn = lambda w: chronos_zero_shot_predictions(w, "amazon/chronos-bolt-small")
            elif model == "chronos-t5-small":
                fn = lambda w: chronos_zero_shot_predictions(w, "amazon/chronos-t5-small")
            elif model == "timesfm-2.5-200m":
                fn = timesfm_zero_shot_predictions
            else:
                print(f"unknown model {model}", file=sys.stderr)
                continue
            try:
                m, wql, elapsed = eval_pretrained(model, fn, windows)
                print(f"| {judge} | {model} (zero-shot) | {len(windows)} | {m:.4f} | {wql:.5f} | {elapsed:.1f} |")
            except Exception as e:
                print(f"| {judge} | {model} (zero-shot) | {len(windows)} | FAILED | FAILED | - |")
                print(f"ERROR running {model} on judge {judge}: {e}", file=sys.stderr)


if __name__ == "__main__":
    main()
