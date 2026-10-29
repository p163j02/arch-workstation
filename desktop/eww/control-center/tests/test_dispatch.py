#!/usr/bin/env python3
"""No-device tests for persistent dispatch and bounded slider queues."""
import importlib.util
from pathlib import Path
import sys
import threading
import unittest

SCRIPTS=Path(__file__).resolve().parents[1]/'scripts'
sys.path.insert(0,str(SCRIPTS))
spec=importlib.util.spec_from_file_location('dispatch',SCRIPTS/'dispatch-service.py')
dispatch=importlib.util.module_from_spec(spec);spec.loader.exec_module(dispatch)

class DispatchTests(unittest.TestCase):
    def test_invalid_requests(self):
        for request in [None,{}, {'kind':'popup','args':['nope']},
                        {'kind':'action','args':['volume','set','nan']},
                        {'kind':'action','args':['volume','set','101']},
                        {'kind':'action','args':['volume','step','20']},
                        {'kind':'action','args':['volume','temperature','3500']}]:
            with self.assertRaises(ValueError):dispatch.validate(request)

    def test_slider_mailbox_keeps_only_latest(self):
        entered=threading.Event();release=threading.Event();done=threading.Event();seen=[]
        def execute(args):
            seen.append(args)
            if len(seen)==1:
                entered.set();release.wait(2)
            else:done.set()
        mailbox=dispatch.SliderMailbox(execute)
        worker=threading.Thread(target=mailbox.worker);worker.start()
        try:
            mailbox.submit(('volume','set','1'))
            self.assertTrue(entered.wait(2))
            for value in range(2,101):mailbox.submit(('volume','set',str(value)))
            self.assertEqual(len(mailbox.pending),1)
            release.set();self.assertTrue(done.wait(2))
            self.assertEqual(seen,[('volume','set','1'),('volume','set','100')])
        finally:
            release.set();mailbox.stop();worker.join(2)

    def test_relative_steps_accumulate(self):
        mailbox=dispatch.SliderMailbox(lambda args:None)
        for _ in range(3):mailbox.submit(('brightness','step','5'))
        self.assertEqual(float(mailbox.pending[('brightness','step')][2]),15)

if __name__=='__main__':unittest.main(verbosity=2)
