#!/usr/bin/env python3
"""Explicit Debug/Release classification; no production feature or timing changes."""
import argparse, hashlib, json, os, pathlib, re, subprocess, sys

PREFIX = 'room_learner::lifecycle::temporal_control::'
C2 = PREFIX + 'tests::c2_all_seven_train_phases_use_actual_async_runtime_and_durable_seals_without_fit'
BUDGET = [PREFIX + 'tests::legacy_329_actual_runtime_saturation_retires_and_explicit_fresh_train_chains_343', PREFIX + 'tests::fresh_343_actual_runtime_all_128_gaps_seven_terminal_pauses_and_retirement_fit_budget']
# This initial policy is populated below from reviewed source call sites.
C3 = ['room_learner::lifecycle::temporal_control::fit_control::tests::c3_actual_native_train_fit_and_same_store_cold_report_keep_whole_model_diagnostic',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_cancel_before_validation_drains_same_job_without_candidate_or_legacy_changes',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_exact_store_recertification_rejects_tampered_head_before_any_fit_save',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_real_model_save_is_drained_before_synthetic_owner_erase_ack_and_never_recreated',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_last_external_owner_drop_drains_real_store_before_return_and_synthetic_erase',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_last_completion_owner_releases_after_signal_on_normal_and_unwind_paths',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_original_expiry_during_fitting_withdraws_publication_without_renewal',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_stale_generation_after_real_save_cannot_adopt_or_strand_job',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_old_completion_cannot_fail_a_replacement_job_with_different_cancel_identity',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_optional_actual_summary_failure_keeps_durable_diagnostic_model',
 'room_learner::lifecycle::temporal_control::fit_control::tests::c3_worker_unwind_keeps_saved_history_but_never_adopts_current_candidate']
C4 = ['room_learner::lifecycle::temporal_control::full_run_tests::c4a_actual_seven_train_fit_frozen_ack_and_exact_cold_model',
 'room_learner::lifecycle::temporal_control::full_run_tests::c4a_actual_frozen_cold_missing_corrupt_wrong_kind_bytes_and_expiry_fail_closed',
 'room_learner::lifecycle::temporal_control::full_run_tests::c4a_owner_cancel_drains_actual_prepared_frozen_writer_before_synthetic_erase',
 'room_learner::lifecycle::temporal_control::full_run_tests::c4a_actual_frozen_lost_ack_same_intent_repair_installs_only_after_exact_ack',
 'room_learner::lifecycle::temporal_control::full_run_tests::c4a_running_compute_cancel_cannot_produce_accepted_fit_or_frozen']

def policy(root):
    source = root / 'v2/crates/wifi-densepose-sensing-server/src/room_learner'
    components = [('C2', source / 'lifecycle_temporal_tests.rs', [C2], [(C2, True, 1)])]
    components += [('C3', source / 'lifecycle_temporal_fit_tests.rs', C3, [(PREFIX + 'fit_control::tests::', False, 12)]), ('C4', source / 'lifecycle_temporal_full_run_tests.rs', C4, [(PREFIX + 'full_run_tests::', False, 8)]), ('budget343', source / 'temporal_budget_profile_tests.rs', BUDGET, [(name, True, 1) for name in BUDGET])]
    active = [item for item in components if item[1].is_file()]
    if not active or active[0][0] != 'C2':
        raise ValueError('C2 classification/source must exist; zero selected tests is not coverage')
    return active

def validate(components, registered, ignored, mode):
    batches = []
    for component, _, classified, selectors in components:
        for name in classified:
            if name not in registered:
                raise ValueError('missing registered classified test: ' + name)
            if (name in ignored) != (mode == 'debug-check'):
                raise ValueError('wrong profile classification for ' + name)
        for selector, exact, minimum in selectors:
            names = sorted(n for n in registered if n == selector) if exact else sorted(n for n in registered if n.startswith(selector))
            if len(names) < minimum:
                raise ValueError(f'{component}: expected >= {minimum} registered tests, got {len(names)}')
            if mode == 'release-run' and any(n in ignored for n in names):
                raise ValueError('an opt-in ignored test would enter a Release batch: ' + selector)
            batches.append({'component': component, 'selector': selector, 'exact': exact, 'names': names})
    return batches

def list_tests(binary, ignored=False):
    result = subprocess.run([str(binary), '--list'] + (['--ignored'] if ignored else []), check=True, capture_output=True, text=True)
    return set(re.findall(r'^(.+): test$', result.stdout, re.M))

def compile_binary(root, release):
    command = ['cargo', 'test', '--locked', '-p', 'wifi-densepose-sensing-server', '--lib', '--no-default-features', '--no-run', '--message-format=json']
    if release:
        command.append('--release')
    result = subprocess.run(command, cwd=root/'v2', capture_output=True, text=True)
    sys.stderr.write(result.stderr)
    binaries = []
    for line in result.stdout.splitlines():
        try: item = json.loads(line)
        except json.JSONDecodeError: continue
        if item.get('reason') == 'compiler-message' and item.get('message', {}).get('rendered'):
            sys.stderr.write(item['message']['rendered'])
        if item.get('reason') == 'compiler-artifact' and item.get('profile', {}).get('test') and item.get('executable') and item.get('target', {}).get('name') == 'wifi_densepose_sensing_server':
            binaries.append(item['executable'])
    if result.returncode:
        raise ValueError('Cargo lib-test compilation failed with exit ' + str(result.returncode))
    if len(set(binaries)) != 1:
        raise ValueError('expected exactly one actual lib-test executable')
    return pathlib.Path(binaries[0])

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['debug-check', 'release-run'])
    parser.add_argument('--evidence-dir', type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    head = os.environ.get('GITHUB_SHA') or subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal', '--ignore-submodules=all'], cwd=root, text=True).strip())
    receipt = {'mode': args.mode, 'head': head, 'worktree_dirty': dirty, 'profile_runner_sha256': hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(), 'batches': [], 'status': 'started'}
    destination = args.evidence_dir / (args.mode + '.json')
    try:
        receipt['rustc'] = subprocess.check_output(['rustc', '-Vv'], cwd=root/'v2', text=True)
        receipt['cargo'] = subprocess.check_output(['cargo', '-V'], cwd=root/'v2', text=True)
        binary = compile_binary(root, args.mode == 'release-run')
        registered = list_tests(binary); ignored = list_tests(binary, True)
        batches = validate(policy(root), registered, ignored, args.mode)
        receipt.update(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), registered_count=len(registered), ignored_names=sorted(ignored), batches=batches)
        print(json.dumps({'mode': args.mode, 'selected_batches': batches}, indent=2), flush=True)
        if args.mode == 'release-run':
            for index, batch in enumerate(batches):
                command = [str(binary), batch['selector'], '--nocapture', '--test-threads=1'] + (['--exact'] if batch['exact'] else [])
                result = subprocess.run(command, cwd=root/'v2', capture_output=True, text=True)
                output = result.stdout + result.stderr
                (args.evidence_dir / f'{index}-{batch["component"]}.log').write_text(output)
                print(output, flush=True)
                counts = re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', output, re.M)
                if result.returncode or not counts or tuple(map(int, counts[-1])) != (len(batch['names']), 0, 0):
                    raise ValueError('Release batch failed or nonzero/exact-count verification failed: ' + batch['selector'])
                batch['passed'] = len(batch['names'])
        receipt['status'] = 'passed'
    except Exception as error:
        receipt.update(status='failed', error=str(error))
        raise
    finally:
        destination.write_text(json.dumps(receipt, indent=2) + '\n')

if __name__ == '__main__':
    main()
