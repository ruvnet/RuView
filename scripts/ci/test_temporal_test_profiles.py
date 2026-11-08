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

if __name__=='__main__':unittest.main()
