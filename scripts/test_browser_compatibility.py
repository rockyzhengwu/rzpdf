"""Integrity tests for corpus aggregation: absence must never become a pass."""
import copy
import unittest
from run_browser_compatibility import aggregate, OPERATIONS


class AggregateTests(unittest.TestCase):
    def setUp(self):
        self.manifest = dict(cases=[dict(id='a', sha256='hash', referencePages=2)])
        events=[]
        for operation in OPERATIONS:
            for page in range(2) if operation in ('render','text') else [None]:
                events.append(dict(operation=operation, page=page, result=dict(status='passed')))
        self.report=dict(manifestSha256='manifest',artifacts={'wasm':'build'},driverSha256='driver',
                         referenceDriverSha256='reference', browser='browser',referenceEngines={},
                         documents=[dict(id='a',sha256='hash',referencePages=2,complete=True,pageCountMismatch=False,
                                         status='passed',operations=events,visual=[dict(page=0,needsReview=False,comparisons=[{},{}]),dict(page=1,needsReview=False,comparisons=[{},{}])])])

    def test_empty_corpus_rejected(self):
        with self.assertRaises(ValueError):aggregate(dict(cases=[]),[])

    def test_unverified_visuals_stay_failed(self):
        self.report['documents'][0]['visual']=[]
        self.assertEqual(aggregate(self.manifest,[self.report])['status'],'failed')

    def test_reference_failure_is_not_a_successful_screening(self):
        self.report['documents'][0]['visual'][0]=dict(page=0,needsReview=True,error='reference exited')
        result=aggregate(self.manifest,[self.report])
        self.assertEqual(result['counts']['visualScreenedPages'],1)
        self.assertEqual(result['counts']['visualReferenceErrorPages'],1)
        self.assertEqual(result['counts']['renderPassed'],2)
        self.assertEqual(result['status'],'failed')

    def test_complete(self):
        result=aggregate(self.manifest,[self.report])
        self.assertTrue(result['complete'])
        self.assertEqual(result['counts']['readableDocuments'],1)
        self.assertEqual(result['status'],'passed')

    def test_missing_page_stays_untested(self):
        doc=self.report['documents'][0]
        doc['operations']=[e for e in doc['operations'] if not(e['operation']=='render' and e['page']==1)]
        result=aggregate(self.manifest,[self.report])
        self.assertEqual(result['counts']['renderUntested'],1)
        self.assertEqual(result['counts']['readableDocuments'],0)
        self.assertFalse(result['complete'])
        self.assertEqual(result['status'],'failed')

    def test_duplicate_document_or_event_rejected(self):
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report,self.report])
        self.report['documents'][0]['operations'].append(self.report['documents'][0]['operations'][0])
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report])

    def test_mixed_build_rejected(self):
        other=copy.deepcopy(self.report);other['artifacts']['wasm']='other';other['documents']=[]
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report,other])

    def test_invalid_page_cannot_replace_missing_page(self):
        next(e for e in self.report['documents'][0]['operations'] if e['operation']=='render')['page']=2
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report])

    def test_mislabelled_failure_stays_failed(self):
        next(e for e in self.report['documents'][0]['operations'] if e['operation']=='render')['result']['status']='failed'
        self.assertEqual(aggregate(self.manifest,[self.report])['status'],'failed')

    def test_mixed_font_policy_rejected(self):
        other=copy.deepcopy(self.report);other['fontConfiguration']=dict(sha256='font');other['documents']=[]
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report,other])

    def test_changed_input_rejected(self):
        self.report['documents'][0]['sha256']='changed'
        with self.assertRaises(ValueError):aggregate(self.manifest,[self.report])


if __name__=='__main__':unittest.main()
