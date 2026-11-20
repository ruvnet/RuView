import argparse
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch
import run_batch as b

class BatchTests(unittest.TestCase):
    def setUp(self):
        gate=patch.object(b,'SUPPORTED_HOST',True); gate.start(); self.addCleanup(gate.stop)

    def receipt(self,state):
        config={'seed':1729,'mode':'identity','identity_protocol':'published_files','evaluate_test':False,
                'device':'cuda','hidden':64,'window':128,'stride':8,'batch_size':64,'max_steps':100000,
                'max_seconds':900.0,'max_train_windows':40000,'max_eval_windows':50000,'threads':2}
        config.update(state.get('experiment',{}))
        return dict(state,status='COMPLETED',test=None,mode='identity',test_evaluation_requested=False,config=config)

    def test_unsupported_host_cannot_spawn(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td))
            with patch.object(b,'SUPPORTED_HOST',False),patch.object(b.subprocess,'Popen') as spawn:
                with self.assertRaisesRegex(ValueError,'requires Linux'): b.run(a)
                spawn.assert_not_called()

    def args(self, root):
        (root/'train.py').write_text('print(1)')
        (root/'data.npz').write_bytes(b'x')
        (root/'data.json').write_text('{}')
        return argparse.Namespace(dataset=root/'data.npz',metadata=root/'data.json',source_root=root,run_root=root/'runs')

    def test_fixed_command_excludes_holdout(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); c=b.command(a,1729,Path(td)/'output')
            self.assertNotIn('--evaluate-test',c)
            self.assertNotIn('--evaluate-checkpoint',c)
            self.assertNotIn('--encoder',c)
            self.assertEqual(c[c.index('--max-seconds')+1],'900')
            self.assertEqual(c[c.index('--window')+1],'128')
            self.assertEqual(c[c.index('--device')+1],'cuda')
            with self.assertRaises(ValueError): b.command(a,999,Path(td)/'output')

    def test_encoder_is_explicit_optional_and_constrained(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td))
            for encoder in ('lstm','gru'):
                a.encoder=encoder
                c=b.command(a,1729,Path(td)/'output')
                self.assertEqual(c[c.index('--encoder')+1],encoder)
                self.assertEqual(c[c.index('--hidden')+1],'64')
                self.assertEqual(c[c.index('--window')+1],'128')
            a.encoder=None
            self.assertNotIn('--encoder',b.command(a,1729,Path(td)/'output'))
            a.encoder='rnn'
            with self.assertRaisesRegex(ValueError,'unsupported encoder'): b.command(a,1729,Path(td)/'output')
            with patch.object(b.subprocess,'Popen') as spawn:
                with self.assertRaisesRegex(ValueError,'unsupported encoder'): b.run(a)
                spawn.assert_not_called()
            with patch('sys.stderr',io.StringIO()), self.assertRaises(SystemExit):
                b.main(['--dataset','unused','--metadata','unused','--source-root','unused','--run-root','unused','--encoder','rnn'])

    def test_exclusive_lock_prevents_spawn(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); (a.run_root/'.batch.lock').mkdir(parents=True)
            with patch.object(b.subprocess,'Popen') as spawn:
                with self.assertRaises(FileExistsError): b.run(a)
                spawn.assert_not_called()
            self.assertTrue((a.run_root/'.batch.lock').is_dir())

    def test_control_budget_validation_precedes_spawn(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); a.representation='demean'; a.temporal_order='shuffle'; a.epochs=50
            c=b.command(a,1729,Path(td)/'output')
            self.assertEqual(c[c.index('--representation')+1],'demean')
            self.assertEqual(c[c.index('--temporal-order')+1],'shuffle')
            self.assertEqual(c[c.index('--epochs')+1],'50')
            for field,value in [('representation','invalid'),('temporal_order','invalid'),('epochs',0),('epochs',201),('epochs',True)]:
                old=getattr(a,field); setattr(a,field,value)
                with patch.object(b.subprocess,'Popen') as spawn:
                    with self.assertRaises(ValueError): b.run(a)
                    spawn.assert_not_called()
                setattr(a,field,old)

    def test_child_control_configuration_must_match(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            state={name+'_sha256':'a'*64 for name in ('trainer','dataset','metadata')}
            state['experiment']={'epochs':50,'representation':'demean','temporal_order':'shuffle','encoder':'gru'}
            metrics=self.receipt(state)
            path=root/'metrics.json'; path.write_text(json.dumps(metrics))
            self.assertEqual(b.verify_metrics(root,state,1729),b.digest(path))
            metrics['config']['temporal_order']='original'; path.write_text(json.dumps(metrics))
            with self.assertRaisesRegex(ValueError,'configuration mismatch'): b.verify_metrics(root,state,1729)
            metrics=self.receipt(state); metrics['config']['epochs']=50.0
            path.write_text(json.dumps(metrics))
            with self.assertRaisesRegex(ValueError,'configuration mismatch: epochs'): b.verify_metrics(root,state,1729)

    def test_wrong_seed_protocol_or_absent_test_cannot_pass(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); state={name+'_sha256':'a'*64 for name in ('trainer','dataset','metadata')}
            for key,value in [('seed',1730),('seed',1729.0),('identity_protocol','session'),('evaluate_test',True),('window',32)]:
                metrics=self.receipt(state); metrics['config'][key]=value
                (root/'metrics.json').write_text(json.dumps(metrics))
                with self.assertRaisesRegex(ValueError,'configuration mismatch'): b.verify_metrics(root,state,1729)
            metrics=self.receipt(state); del metrics['test']
            (root/'metrics.json').write_text(json.dumps(metrics))
            with self.assertRaisesRegex(ValueError,'missing or unexpected'): b.verify_metrics(root,state,1729)

    def test_three_serial_children_and_state(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); child=Mock(pid=123); child.wait.return_value=0; child.poll.return_value=0
            with patch.object(b,'gpu_free',return_value=5000) as gpu, patch.object(b.subprocess,'Popen',return_value=child) as spawn, patch.object(b,'monitor_child',return_value=0), patch.object(b,'verify_metrics',return_value='a'*64), patch.object(b,'stop_child'):
                result=b.run(a)
            self.assertEqual(result['status'],'COMPLETED')
            self.assertEqual([j['seed'] for j in result['jobs']],list(b.SEEDS))
            self.assertEqual(gpu.call_count,3)
            self.assertEqual(spawn.call_count,3)
            self.assertFalse((a.run_root/'.batch.lock').exists())
            for call in spawn.call_args_list:
                self.assertIs(call.kwargs['shell'],False)
                self.assertEqual(call.kwargs['env']['CUDA_VISIBLE_DEVICES'],'0')

    def test_timeout_terminates_kills_reaps_and_stops_batch(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); child=Mock(pid=456); child.poll.return_value=None
            child.wait.side_effect=[subprocess.TimeoutExpired('train',20),-9]
            cleanup=b.stop_child
            with patch.object(b,'gpu_free',return_value=5000), patch.object(b.subprocess,'Popen',return_value=child) as spawn, patch.object(b,'monitor_child',side_effect=subprocess.TimeoutExpired('train',1020)), patch.object(b,'stop_child',side_effect=lambda c,g:cleanup(c,False)):
                with self.assertRaises(subprocess.TimeoutExpired): b.run(a)
            child.terminate.assert_called_once(); child.kill.assert_called_once()
            self.assertEqual(spawn.call_count,1)
            self.assertFalse((a.run_root/'.batch.lock').exists())
            state=json.loads(next(a.run_root.glob('batch-*/batch-state.json')).read_text())
            self.assertEqual(state['status'],'INCOMPLETE')
            self.assertEqual(state['jobs'][0]['pid'],456)
            self.assertEqual(state['jobs'][0]['error_type'],'TimeoutExpired')

    def test_gpu_gate_is_bounded_and_rejects_low_memory(self):
        for value in ['4095','N/A','5000\n6000']:
            with patch.object(b.subprocess,'run',return_value=Mock(stdout=value)) as call:
                with self.assertRaises(ValueError): b.gpu_free()
                self.assertEqual(call.call_args.kwargs['timeout'],15)

    def test_log_bytes_are_hard_capped(self):
        log=io.BytesIO(); errors=[]
        b.drain_log(io.BytesIO(b'x'*1000),log,errors,limit=100)
        self.assertEqual(len(log.getvalue()),100)
        self.assertEqual(errors,['child log limit exceeded'])

    def test_metrics_hash_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            state={name+'_sha256':'a'*64 for name in ('trainer','dataset','metadata')}
            metrics=self.receipt(state)
            metrics['metadata_sha256']='b'*64
            (root/'metrics.json').write_text(json.dumps(metrics))
            with self.assertRaisesRegex(ValueError,'metadata_sha256 mismatch'): b.verify_metrics(root,state,1729)

    def test_metrics_above_one_mib_are_read_below_eight_mib(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            state={name+'_sha256':'a'*64 for name in ('trainer','dataset','metadata')}
            metrics=self.receipt(state); metrics['history']='x'*(2*1024*1024)
            path=root/'metrics.json'
            path.write_text(json.dumps(metrics))
            self.assertEqual(b.verify_metrics(root,state,1729),b.digest(path))
            path.write_bytes(b'x'*(b.MAX_METRICS_BYTES+1))
            with self.assertRaisesRegex(ValueError,'oversized'): b.verify_metrics(root,state,1729)

    def test_subset_resumes_only_remaining_fixed_seeds(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td)); a.seeds=[1730,1731]
            child=Mock(pid=123); child.wait.return_value=0; child.poll.return_value=0
            with patch.object(b,'gpu_free',return_value=5000), patch.object(b.subprocess,'Popen',return_value=child) as spawn, patch.object(b,'monitor_child',return_value=0), patch.object(b,'verify_metrics',return_value='a'*64), patch.object(b,'stop_child'):
                result=b.run(a)
            self.assertEqual([j['seed'] for j in result['jobs']],[1730,1731])
            self.assertEqual(result['requested_seeds'],[1730,1731])
            self.assertEqual(spawn.call_count,2)

    def test_duplicate_or_unapproved_seeds_never_spawn(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td))
            for seeds in ([],[1729,1729],[999],[True],['1729']):
                a.seeds=seeds
                with patch.object(b.subprocess,'Popen') as spawn:
                    with self.assertRaisesRegex(ValueError,'unique subset'): b.run(a)
                    spawn.assert_not_called()

    def test_input_mutation_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            a=self.args(Path(td))
            state={name+'_sha256':b.digest(path) for name,path in [('trainer',a.source_root/'train.py'),('dataset',a.dataset),('metadata',a.metadata)]}
            a.metadata.write_text('{"changed":true}')
            with self.assertRaisesRegex(ValueError,'metadata changed'): b.verify_inputs(a,state)

    def test_linux_group_cleanup_only_owned_group(self):
        child=Mock(pid=1234); child.poll.return_value=None; child.wait.return_value=0
        with patch.object(b.os,'killpg',create=True) as killpg, patch.object(b.signal,'SIGKILL',9,create=True):
            b.stop_child(child,True)
        self.assertEqual({call.args[0] for call in killpg.call_args_list},{1234})
        child.terminate.assert_not_called()
        child.kill.assert_not_called()

    def test_monitor_terminates_on_log_cap(self):
        child=Mock(pid=1234); child.stdout=io.BytesIO(b'x'*20); child.poll.return_value=None
        def reader(pipe,log,errors):
            b.drain_log_original(pipe,log,errors,limit=10)
        with patch.object(b,'drain_log_original',b.drain_log,create=True), patch.object(b,'drain_log',side_effect=reader), patch.object(b,'stop_child') as stop:
            with self.assertRaisesRegex(ValueError,'log limit'): b.monitor_child(child,io.BytesIO(),False)
            stop.assert_called()

if __name__=='__main__': unittest.main()
