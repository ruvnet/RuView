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
if __name__=='__main__':unittest.main()
