#!/usr/bin/env python3
"""Explicit Debug/Release classification; no production feature or timing changes."""
import argparse, hashlib, json, os, pathlib, re, stat, subprocess, sys

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

C4B_PREFIX = PREFIX + "held_out_tests::"
C4B = ['room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_cold_restore_requires_model_owner_context_and_original_deadline', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_owner_cancel_and_retirement_deny_reactivation', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_rehashed_role_phase_binding_floor_and_deadline_negatives', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_native_train_frozen_eight_heldout_two_cold_interruptions_complete']
C4B_ALL = ['room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_cold_restore_requires_model_owner_context_and_original_deadline', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_owner_cancel_and_retirement_deny_reactivation', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_complete_rehashed_role_phase_binding_floor_and_deadline_negatives', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_actual_native_train_frozen_eight_heldout_two_cold_interruptions_complete', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_full2_abort_contract_is_denied_before_any_parent_ticket_operation', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_full2_fence_budget_exact_boundary_and_other_guards_remain_strict', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_full2_one_inflight_one_tail_no_third_and_same_repair_adopts_queued_window', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_full_v3_cannot_adopt_kind1_fresh343_train_policy', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_native_partial_rollback_reproduces_retained_only_frame_fence_assumption', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_old_kind1_status_shape_has_no_full2_role_fields', 'room_learner::lifecycle::temporal_control::held_out_tests::c4b_rejected_native_packet_adds_no_fence_and_terminal_state_cannot_accumulate']

def policy(root):
    source = root / 'v2/crates/wifi-densepose-sensing-server/src/room_learner'
    components = [('C2', source / 'lifecycle_temporal_tests.rs', [C2], [(C2, True, 1)])]
    components += [('C3', source / 'lifecycle_temporal_fit_tests.rs', C3, [(PREFIX + 'fit_control::tests::', False, 12)]), ('C4', source / 'lifecycle_temporal_full_run_tests.rs', C4, [(PREFIX + 'full_run_tests::', False, 8)]), ('budget343', source / 'temporal_budget_profile_tests.rs', BUDGET, [(name, True, 1) for name in BUDGET])]
    b_marker = source / 'lifecycle_temporal_held_out_tests.rs'
    if not b_marker.is_file():
        raise ValueError('C4B source marker is required; missing source is not coverage')
    components += [('C4B', b_marker, C4B, [(C4B_PREFIX, False, 11)])]
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
            if component == 'C4B':
                if set(names) != set(C4B_ALL) or len(names) != 11:
                    raise ValueError('C4B must register exactly the reviewed eleven tests')
                if mode == 'debug-check' and any(n in ignored for n in set(C4B_ALL) - set(C4B)):
                    raise ValueError('C4B small controls must stay Debug-active')
            if len(names) < minimum:
                raise ValueError(f'{component}: expected >= {minimum} registered tests, got {len(names)}')
            if mode == 'release-run' and any(n in ignored for n in names):
                raise ValueError('an opt-in ignored test would enter a Release batch: ' + selector)
            batches.append({'component': component, 'selector': selector, 'exact': exact, 'names': names})
    return batches

def list_tests(binary, ignored=False):
    result = subprocess.run([str(binary), '--list'] + (['--ignored'] if ignored else []), check=True, capture_output=True, text=True)
    return set(re.findall(r'^(.+): test$', result.stdout, re.M))

def validate_executable(path, target_directory, release):
    raw = pathlib.Path(path)
    target_raw = pathlib.Path(target_directory)
    if not raw.is_absolute() or not target_raw.is_absolute():
        raise ValueError('Cargo artifact and target directory must be absolute')
    target = target_raw.resolve(strict=True)
    profile = 'release' if release else 'debug'
    expected = target / profile / 'deps'
    # The configured target root may be external/canonicalized by Cargo; below
    # that root neither profile, deps nor executable may redirect through links.
    for directory in [target_raw / profile, target_raw / profile / 'deps']:
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError('unsafe Cargo profile/deps directory')
    if raw.parent not in (target_raw / profile / 'deps', expected):
        raise ValueError('Cargo executable outside declared profile/deps')
    info = raw.lstat()
    if not stat.S_ISREG(info.st_mode) or raw.is_symlink() or not os.access(raw, os.X_OK):
        raise ValueError('Cargo executable must be regular, executable and not a symlink')
    if not re.fullmatch(r'wifi_densepose_sensing_server-[0-9a-f]{16}', raw.name):
        raise ValueError('unexpected lib-test executable basename')
    resolved = raw.resolve(strict=True)
    if resolved.parent != expected.resolve(strict=True):
        raise ValueError('Cargo executable escaped canonical target profile/deps')
    return resolved

def validate_cargo_artifacts(root, metadata, artifacts, release):
    manifest = (root / 'v2/crates/wifi-densepose-sensing-server/Cargo.toml').resolve(strict=True)
    source = (manifest.parent / 'src/lib.rs').resolve(strict=True)
    packages = [p for p in metadata.get('packages', []) if p.get('name') == 'wifi-densepose-sensing-server' and pathlib.Path(p.get('manifest_path', '')).resolve() == manifest]
    if len(packages) != 1:
        raise ValueError('expected exactly one workspace sensing-server package')
    package = packages[0]
    libraries = [t for t in package.get('targets', []) if t.get('name') == 'wifi_densepose_sensing_server' and t.get('kind') == ['lib'] and pathlib.Path(t.get('src_path', '')).resolve() == source]
    if len(libraries) != 1:
        raise ValueError('expected exactly one actual workspace library source')
    candidates = [a for a in artifacts if a.get('reason') == 'compiler-artifact' and a.get('executable') and a.get('package_id') == package['id'] and a.get('target', {}).get('name') == 'wifi_densepose_sensing_server' and a.get('profile', {}).get('test') is True]
    if len(candidates) != 1:
        raise ValueError('expected exactly one actual package/lib-test executable')
    artifact = candidates[0]
    target = artifact.get('target', {})
    profile = artifact.get('profile', {})
    if target.get('kind') != ['lib'] or pathlib.Path(target.get('src_path', '')).resolve() != source or profile.get('opt_level') != ('3' if release else '0') or profile.get('debug_assertions') is not (not release):
        raise ValueError('Cargo executable library source or execution profile mismatch')
    executable = validate_executable(artifact['executable'], metadata['target_directory'], release)
    return {'path': executable, 'sha256': hashlib.sha256(executable.read_bytes()).hexdigest(), 'target_directory': pathlib.Path(metadata['target_directory']).resolve(strict=True), 'package_id': package['id'], 'lib_source': source, 'release': release}

def verify_selected_executable(selected, expected_sha256):
    binary = validate_executable(selected['path'], selected['target_directory'], selected['release'])
    if hashlib.sha256(binary.read_bytes()).hexdigest() != expected_sha256:
        raise ValueError('selected Cargo executable changed before invocation')
    return binary

def compile_binary(root, release):
    metadata_result = subprocess.run(['cargo', 'metadata', '--locked', '--no-deps', '--format-version=1', '--no-default-features'], cwd=root/'v2', check=True, capture_output=True, text=True)
    metadata = json.loads(metadata_result.stdout)
    command = ['cargo', 'test', '--locked', '-p', 'wifi-densepose-sensing-server', '--lib', '--no-default-features', '--no-run', '--message-format=json']
    if release:
        command.append('--release')
    result = subprocess.run(command, cwd=root/'v2', capture_output=True, text=True)
    sys.stderr.write(result.stderr)
    artifacts = []
    for line in result.stdout.splitlines():
        try: item = json.loads(line)
        except json.JSONDecodeError: continue
        if item.get('reason') == 'compiler-message' and item.get('message', {}).get('rendered'):
            sys.stderr.write(item['message']['rendered'])
        if item.get('reason') == 'compiler-artifact':
            artifacts.append(item)
    if result.returncode:
        raise ValueError('Cargo lib-test compilation failed with exit ' + str(result.returncode))
    return validate_cargo_artifacts(root, metadata, artifacts, release)

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
        if args.mode == 'release-run':
            selected = compile_binary(root, True)
        else:
            selected = compile_binary(root, False)
        binary = selected['path']
        binary_sha256 = selected['sha256']
        verify_selected_executable(selected, binary_sha256)
        registered = list_tests(binary)
        verify_selected_executable(selected, binary_sha256)
        ignored = list_tests(binary, True)
        verify_selected_executable(selected, binary_sha256)
        if args.mode == 'release-run':
            batches = validate(policy(root), registered, ignored, 'release-run')
        else:
            batches = validate(policy(root), registered, ignored, 'debug-check')
        receipt.update(binary=str(binary), binary_sha256=binary_sha256, cargo_artifact={'target_directory': str(selected['target_directory']), 'package_id': selected['package_id'], 'lib_source': str(selected['lib_source']), 'profile': 'release' if selected['release'] else 'debug'}, registered_count=len(registered), ignored_names=sorted(ignored), batches=batches)
        print(json.dumps({'mode': args.mode, 'selected_batches': batches}, indent=2), flush=True)
        if args.mode == 'release-run':
            for index, batch in enumerate(batches):
                verify_selected_executable(selected, binary_sha256)
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
