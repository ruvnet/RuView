import json
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import codex_iteration as c


class ReviewBoundaryTests(unittest.TestCase):
    def test_snapshot_allows_only_bfi_source_and_binds_hash(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td);(root/'.git').mkdir();(root/'AGENTS.md').write_text('contract')
            tools=root/'tools/bfi';tools.mkdir(parents=True);(tools/'a.py').write_text('x=1')
            self.assertEqual(c.snapshot(root,['tools/bfi/a.py'])[0]['content'],'x=1')
            (root/'private.env').write_text('not for review')
            with self.assertRaises(ValueError):c.snapshot(root,['private.env'])
            with self.assertRaises(ValueError):c.snapshot(root,[])
    def test_private_output_rejects_checkout(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td).resolve();(root/'.git').mkdir()
            with self.assertRaises(ValueError):c.checked_output_root(root/'logs',root)
    def test_child_policy_keeps_exec_rules_and_read_only(self):
        cmd=c.command(Path('codex'),Path('repo'))
        self.assertIn('read-only',cmd)
        self.assertNotIn('--ignore-rules',cmd)
        self.assertFalse(any('bypass' in a for a in cmd))
        self.assertIn('shell_tool',cmd)
    def test_review_rejects_tools_and_malformed_events(self):
        with tempfile.TemporaryDirectory() as td:
            p=Path(td)/'events.jsonl'
            p.write_text(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':'review'}})+'\n'+json.dumps({'type':'turn.completed'})+'\n')
            self.assertEqual(c.audit_events(p),(['review'],[]))
            with p.open('a') as f:
                f.write(json.dumps({'type':'item.started','item':{'type':'command_execution'}})+'\ninvalid\n')
            findings,errors=c.audit_events(p)
            self.assertEqual(findings,['review'])
            self.assertEqual([e['type'] for e in errors],['prohibited_activity','invalid_json_event'])
            p.write_text('{"type":null}\n')
            self.assertEqual([e['type'] for e in c.audit_events(p)[1]],['invalid_event_type','missing_turn_completed'])
    def test_source_bound_and_nonregular_file(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td);(root/'.git').mkdir();(root/'AGENTS.md').write_text('contract')
            folder=root/'tools/bfi';folder.mkdir(parents=True)
            (folder/'large.py').write_bytes(b'x'*(c.MAX_SOURCE+1))
            with self.assertRaises(ValueError):c.snapshot(root,['tools/bfi/large.py'])
            (folder/'directory.py').mkdir()
            with self.assertRaises(ValueError):c.snapshot(root,['tools/bfi/directory.py'])
    def test_output_failure_reaps_child(self):
        class BrokenSink:
            def write(self,data):raise OSError('simulated disk full')
        original=c.subprocess.Popen;children=[]
        def spawn(*a,**k):
            child=original(*a,**k);children.append(child);return child
        program="import sys,time;sys.stdout.buffer.write(b'x'*8192);sys.stdout.flush();time.sleep(20)"
        with patch.object(c,'command',return_value=[sys.executable,'-c',program]), patch.object(c.subprocess,'Popen',side_effect=spawn):
            with self.assertRaises(OSError):
                c.run_with_sinks(Path(sys.executable),Path('.'),'input',5,{'stdout':BrokenSink(),'stderr':io.BytesIO()})
        self.assertIsNotNone(children[0].poll())
    def test_stdin_receipt_and_deadline(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td);ok=root/'ok';ok.mkdir()
            program="import sys,json; x=sys.stdin.read(); print(json.dumps({'received':len(x)}))"
            with patch.object(c,'command',return_value=[sys.executable,'-c',program]):
                result=c.run(Path(sys.executable),root,ok,'abc',5)
            self.assertEqual(result['exit_code'],0)
            self.assertEqual(json.loads((ok/'events.jsonl').read_text())['received'],3)
            slow=root/'slow';slow.mkdir()
            with patch.object(c,'command',return_value=[sys.executable,'-c','import time;time.sleep(10)']):
                result=c.run(Path(sys.executable),root,slow,'x'*240000,.25)
            self.assertEqual(result['stop_reason'],'timeout')
            self.assertLess(result['elapsed_seconds'],3)


if __name__=='__main__':unittest.main()
