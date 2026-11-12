import importlib.util, pathlib, unittest
spec=importlib.util.spec_from_file_location('profiles',pathlib.Path(__file__).with_name('temporal_test_profiles.py'));profiles=importlib.util.module_from_spec(spec);spec.loader.exec_module(profiles)
class ClassificationTests(unittest.TestCase):
    def component(self):return [('C2',None,[profiles.C2],[(profiles.C2,True,1)])]
    def test_missing_registered_test_never_counts_as_coverage(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(),set(),'release-run')
    def test_debug_requires_transparent_ignored_classification(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.C2},set(),'debug-check')
    def test_debug_classification_keeps_registered_name(self):
        result=profiles.validate(self.component(),{profiles.C2},{profiles.C2},'debug-check');self.assertEqual(result[0]['names'],[profiles.C2])
    def test_release_case_is_present_active_and_nonzero(self):
        result=profiles.validate(self.component(),{profiles.C2},set(),'release-run');self.assertEqual(len(result),1);self.assertTrue(result[0]['exact'])
    def test_release_never_sweeps_ignored_optin_performance(self):
        component=[('fixture',None,['module::positive'],[('module::',False,1)])]
        with self.assertRaises(ValueError):profiles.validate(component,{'module::positive','module::manual_perf'},{'module::manual_perf'},'release-run')
    def test_module_is_one_batch_to_preserve_fixture_cache(self):
        component=[('fixture',None,['module::one'],[('module::',False,2)])]
        result=profiles.validate(component,{'module::one','module::two'},set(),'release-run');self.assertEqual(len(result),1);self.assertEqual(result[0]['names'],['module::one','module::two'])


class C1ClassificationTests(unittest.TestCase):
    def component(self):
        return [next(item for item in profiles.policy(pathlib.Path(__file__).resolve().parents[2]) if item[0] == 'C1')]
    def test_current_policy_requires_one_exact_release_reference(self):
        result=profiles.validate(self.component(),{profiles.C1,profiles.C1+'_lookalike',profiles.C2},set(),'release-run')
        self.assertEqual(result,[{'component':'C1','selector':profiles.C1,'exact':True,'names':[profiles.C1]}])
    def test_reference_stays_registered_but_transparently_ignored_in_debug(self):
        result=profiles.validate(self.component(),{profiles.C1},{profiles.C1},'debug-check');self.assertEqual(result[0]['names'],[profiles.C1])
    def test_missing_reference_cannot_be_silently_omitted(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(),set(),'release-run')
    def test_reference_cannot_run_unclassified_in_debug(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.C1},set(),'debug-check')
    def test_reference_cannot_be_ignored_in_required_release(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.C1},{profiles.C1},'release-run')
    def test_zero_or_lookalike_only_is_not_the_exact_reference(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.C1+'_manual_perf'},set(),'release-run')


class C4BClassificationTests(unittest.TestCase):
    def component(self):return [('C4B',None,profiles.C4B,[(profiles.C4B_PREFIX,False,11)])]
    def test_exact_eleven_release_is_one_batch_with_fixture_cache(self):
        batches=profiles.validate(self.component(),set(profiles.C4B_ALL),set(),'release-run');self.assertEqual(len(batches),1);self.assertEqual(len(batches[0]['names']),11);self.assertFalse(batches[0]['exact'])
    def test_debug_keeps_seven_small_active_and_four_heavy_transparent(self):
        batches=profiles.validate(self.component(),set(profiles.C4B_ALL),set(profiles.C4B),'debug-check');self.assertEqual(len(batches[0]['names']),11);self.assertEqual(len(set(profiles.C4B_ALL)-set(profiles.C4B)),7)
    def test_missing_heavy_never_counts_as_coverage(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(profiles.C4B_ALL)-{profiles.C4B[0]},set(),'release-run')
    def test_extra_unclassified_test_requires_review(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(profiles.C4B_ALL)|{profiles.C4B_PREFIX+'unexpected'},set(),'release-run')
    def test_fourth_heavy_must_not_run_in_debug(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(profiles.C4B_ALL),set(profiles.C4B[:-1]),'debug-check')
    def test_small_controls_cannot_be_hidden_in_debug(self):
        small=next(n for n in profiles.C4B_ALL if n not in profiles.C4B)
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(profiles.C4B_ALL),set(profiles.C4B)|{small},'debug-check')
    def test_no_ignored_or_opt_in_release_selection(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(profiles.C4B_ALL),{profiles.C4B[0]},'release-run')
    def test_zero_tests_and_missing_source_marker_fail_closed(self):
        import tempfile
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(),set(),'release-run')
        with tempfile.TemporaryDirectory() as d:
            root=pathlib.Path(d);p=root/'v2/crates/wifi-densepose-sensing-server/src/room_learner/lifecycle_temporal_tests.rs';p.parent.mkdir(parents=True);p.write_text('// synthetic marker only')
            with self.assertRaises(ValueError):profiles.policy(root)

class CargoExecutableCustodyTests(unittest.TestCase):
    def setUp(self):
        import tempfile, copy
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.base=pathlib.Path(self.temp.name).resolve();self.root=self.base/'workspace';self.target=self.base/'external-warm-target'
        self.manifest=self.root/'v2/crates/wifi-densepose-sensing-server/Cargo.toml';self.manifest.parent.mkdir(parents=True);self.manifest.write_text('[package]\nname="synthetic"\n')
        self.source=self.manifest.parent/'src/lib.rs';self.source.parent.mkdir();self.source.write_text('// synthetic library source\n')
        self.binary=self.target/'release/deps/wifi_densepose_sensing_server-0123456789abcdef';self.binary.parent.mkdir(parents=True);self.binary.write_text('#!/bin/sh\nexit 0\n');self.binary.chmod(0o700)
        self.lib={'name':'wifi_densepose_sensing_server','kind':['lib'],'src_path':str(self.source)}
        self.metadata={'target_directory':str(self.target),'packages':[{'name':'wifi-densepose-sensing-server','manifest_path':str(self.manifest),'id':'synthetic-workspace-package','targets':[self.lib]}]}
        self.artifact={'reason':'compiler-artifact','package_id':'synthetic-workspace-package','target':copy.deepcopy(self.lib),'profile':{'test':True,'opt_level':'3','debug_assertions':False},'executable':str(self.binary)}
    def valid(self,artifacts=None):
        return profiles.validate_cargo_artifacts(self.root,self.metadata,artifacts if artifacts is not None else [self.artifact],True)
    def test_actual_external_warm_target_is_preserved(self):
        result=self.valid();self.assertEqual(result['path'],self.binary);self.assertEqual(result['target_directory'],self.target)
    def test_executable_outside_reported_target_is_rejected(self):
        outside=self.base/self.binary.name;outside.write_bytes(self.binary.read_bytes());outside.chmod(0o700);self.artifact['executable']=str(outside)
        with self.assertRaises(ValueError):self.valid()
    def test_symlink_executable_is_rejected(self):
        original=self.binary.with_name('original');self.binary.rename(original);self.binary.symlink_to(original)
        with self.assertRaises(ValueError):self.valid()
    def test_wrong_profile_artifact_is_rejected(self):
        self.artifact['profile']['opt_level']='0';self.artifact['profile']['debug_assertions']=True
        with self.assertRaises(ValueError):self.valid()
    def test_wrong_library_source_is_rejected(self):
        wrong=self.source.with_name('wrong.rs');wrong.write_text('// wrong source\n');self.artifact['target']['src_path']=str(wrong)
        with self.assertRaises(ValueError):self.valid()
    def test_wrong_package_is_rejected(self):
        self.artifact['package_id']='other-package'
        with self.assertRaises(ValueError):self.valid()
    def test_wrong_target_kind_is_rejected(self):
        self.artifact['target']['kind']=['bin']
        with self.assertRaises(ValueError):self.valid()
    def test_duplicate_test_artifacts_are_rejected(self):
        with self.assertRaises(ValueError):self.valid([self.artifact,self.artifact])
    def test_nonexecutable_regular_file_is_rejected(self):
        self.binary.chmod(0o600)
        with self.assertRaises(ValueError):self.valid()


    def test_swapped_bytes_after_admission_are_not_invoked(self):
        import hashlib
        selected=self.valid();digest=hashlib.sha256(self.binary.read_bytes()).hexdigest();self.binary.write_text('#!/bin/sh\necho changed\n')
        with self.assertRaises(ValueError):profiles.verify_selected_executable(selected,digest)
    def test_symlink_swap_after_admission_is_not_invoked(self):
        import hashlib
        selected=self.valid();digest=hashlib.sha256(self.binary.read_bytes()).hexdigest();other=self.base/'outside';other.write_bytes(self.binary.read_bytes());other.chmod(0o700);self.binary.unlink();self.binary.symlink_to(other)
        with self.assertRaises(ValueError):profiles.verify_selected_executable(selected,digest)


    def test_actual_main_rejects_replacement_before_any_registry_execution(self):
        from unittest import mock
        import sys
        selected=self.valid();marker=self.base/'outside-executed';outside=self.base/'outside-libtest'
        outside.write_text('#!/bin/sh\nprintf executed > '+str(marker)+'\n');outside.chmod(0o700)
        def after_compile(root,release):
            self.binary.unlink();self.binary.symlink_to(outside);return selected
        with mock.patch.object(sys,'argv',['profiles','release-run','--evidence-dir',str(self.base/'receipt')]), mock.patch.object(profiles,'compile_binary',side_effect=after_compile), mock.patch.object(profiles.subprocess,'check_output',return_value='synthetic-provenance'):
            with self.assertRaises(ValueError):profiles.main()
        self.assertFalse(marker.exists(),'registry executed substituted out-of-target artifact')

class C4CClassificationTests(unittest.TestCase):
    def component(self):
        return [('C4C', None, [profiles.C4C], [(profiles.C4C, True, 1)])]
    def test_exact_default48_case_is_one_required_release_case(self):
        result = profiles.validate(self.component(), {profiles.C4C, profiles.C4C + '_lookalike'}, set(), 'release-run')
        self.assertEqual(result, [{'component': 'C4C', 'selector': profiles.C4C, 'exact': True, 'names': [profiles.C4C]}])
    def test_debug_requires_the_same_case_registered_and_transparently_ignored(self):
        result = profiles.validate(self.component(), {profiles.C4C}, {profiles.C4C}, 'debug-check')
        self.assertEqual(result[0]['names'], [profiles.C4C])
    def test_default48_cannot_be_left_unclassified_in_debug(self):
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {profiles.C4C}, set(), 'debug-check')
    def test_default48_cannot_be_ignored_in_required_release(self):
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {profiles.C4C}, {profiles.C4C}, 'release-run')
    def test_zero_or_missing_case_is_not_release_coverage(self):
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), set(), set(), 'release-run')
    def test_same_leaf_in_a_different_module_cannot_satisfy_exact_registration(self):
        wrong = profiles.C4C.replace('public_workflow::acceptance::', 'public_workflow::wrong_module::')
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {wrong}, set(), 'release-run')
    def test_exact_selector_does_not_sweep_opt_in_siblings(self):
        perf = profiles.C4C + '_manual_performance'
        result = profiles.validate(self.component(), {profiles.C4C, perf}, {perf}, 'release-run')
        self.assertEqual(result[0]['names'], [profiles.C4C])
    def test_missing_c4c_source_marker_cannot_silently_drop_release_batch(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            source = root / 'v2/crates/wifi-densepose-sensing-server/src/room_learner'
            source.mkdir(parents=True)
            for name in ['lifecycle_temporal_tests.rs', 'lifecycle_temporal_held_out_tests.rs']:
                (source / name).write_text('// synthetic source marker only')
            with self.assertRaisesRegex(ValueError, 'C4C acceptance source marker'):
                profiles.policy(root)
    def test_current_policy_registers_exact_c4c_and_preserves_existing_components(self):
        components = profiles.policy(pathlib.Path(__file__).resolve().parents[2])
        self.assertEqual([c[0] for c in components], ['C2', 'C1', 'C3', 'C4', 'budget343', 'C4B', 'C4C', 'variableRetry'])
        current = next(c for c in components if c[0] == 'C4C')
        self.assertEqual(current[1].name, 'lifecycle_temporal_public_acceptance_tests.rs')
        self.assertEqual(current[2:], ([profiles.C4C], [(profiles.C4C, True, 1)]))
        self.assertEqual(components[0][3], [(profiles.C2, True, 1)])
        self.assertEqual(components[1][3], [(profiles.C1, True, 1)])
        self.assertEqual(components[2][3], [(profiles.PREFIX + 'fit_control::tests::', False, 12)])
        self.assertEqual(components[3][3], [(profiles.PREFIX + 'full_run_tests::', False, 9)])
        self.assertEqual(components[4][3], [(name, True, 1) for name in profiles.BUDGET])
        self.assertEqual(components[5][3], [(profiles.C4B_PREFIX, False, 11)])



class OperatorEightClassificationTests(unittest.TestCase):
    def component(self):return [('operator8',None,[profiles.OPERATOR8],[(profiles.OPERATOR8,True,1)])]
    def test_exact_operator_case_is_mandatory_active_in_release(self):
        result=profiles.validate(self.component(),{profiles.OPERATOR8,profiles.OPERATOR8+'_lookalike'},set(),'release-run')
        self.assertEqual(result[0]['names'],[profiles.OPERATOR8])
    def test_debug_registration_is_explicitly_ignored_not_passed(self):
        result=profiles.validate(self.component(),{profiles.OPERATOR8},{profiles.OPERATOR8},'debug-check')
        self.assertEqual(result[0]['names'],[profiles.OPERATOR8])
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.OPERATOR8},set(),'debug-check')
    def test_missing_or_ignored_release_case_fails(self):
        with self.assertRaises(ValueError):profiles.validate(self.component(),set(),set(),'release-run')
        with self.assertRaises(ValueError):profiles.validate(self.component(),{profiles.OPERATOR8},{profiles.OPERATOR8},'release-run')
    def test_actual_policy_carries_operator_case_in_cached_c4_batch(self):
        c4=next(c for c in profiles.policy(pathlib.Path(__file__).resolve().parents[2]) if c[0]=='C4')
        self.assertIn(profiles.OPERATOR8,c4[2]);self.assertEqual(c4[3],[(profiles.PREFIX+'full_run_tests::',False,9)])

class VariableRetryClassificationTests(unittest.TestCase):
    def component(self):
        root = pathlib.Path(__file__).resolve().parents[2]
        return [next(c for c in profiles.policy(root) if c[0] == 'variableRetry')]

    def test_current_policy_requires_exact_native_retry_case(self):
        c = self.component()[0]
        self.assertEqual(c[1].name, 'lifecycle_temporal_tests.rs')
        self.assertEqual(c[2:], ([profiles.VARIABLE_RETRY], [(profiles.VARIABLE_RETRY, True, 1)]))
        self.assertIn('fn variable_retry_after_actual_window_cadence_failure_keeps_sixteen_windows_and_seal()', c[1].read_text())

    def test_debug_keeps_the_case_registered_and_transparently_ignored(self):
        result = profiles.validate(self.component(), {profiles.VARIABLE_RETRY}, {profiles.VARIABLE_RETRY}, 'debug-check')
        self.assertEqual(result[0]['names'], [profiles.VARIABLE_RETRY])

    def test_debug_cannot_run_the_native_persistence_case_unclassified(self):
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {profiles.VARIABLE_RETRY}, set(), 'debug-check')

    def test_release_requires_one_active_case_and_does_not_sweep_siblings(self):
        sibling = profiles.VARIABLE_RETRY + '_manual_performance'
        result = profiles.validate(self.component(), {profiles.VARIABLE_RETRY, sibling}, {sibling}, 'release-run')
        self.assertEqual(result, [{'component': 'variableRetry', 'selector': profiles.VARIABLE_RETRY, 'exact': True, 'names': [profiles.VARIABLE_RETRY]}])

    def test_missing_or_lookalike_case_cannot_satisfy_release_coverage(self):
        for registered in [set(), {profiles.VARIABLE_RETRY + '_lookalike'}]:
            with self.assertRaises(ValueError):
                profiles.validate(self.component(), registered, set(), 'release-run')

    def test_ignored_release_case_cannot_be_counted_as_passed(self):
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {profiles.VARIABLE_RETRY}, {profiles.VARIABLE_RETRY}, 'release-run')

    def test_wrong_module_cannot_substitute_the_same_leaf(self):
        wrong = profiles.VARIABLE_RETRY.replace('tests::', 'wrong_module::')
        with self.assertRaises(ValueError):
            profiles.validate(self.component(), {wrong}, set(), 'release-run')


if __name__=='__main__':unittest.main()
