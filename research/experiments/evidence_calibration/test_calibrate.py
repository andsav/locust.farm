import unittest
from calibrate import public, score, select

class CalibrationTests(unittest.TestCase):
    def row(self):
        return dict(id='3hop__a',question='Q',answer='The Test',answer_aliases=['test'],answerable=True,
                    paragraphs=[dict(idx=1,title='T',paragraph_text='P',is_supporting=True)],
                    question_decomposition=[{'answer':'SECRET'}])
    def test_no_label_leakage(self):
        self.assertEqual(public(self.row()),{'question':'Q','documents':[{'id':1,'title':'T','text':'P'}]})
    def test_answer_does_not_imply_support(self):
        r={'status':'completed','text':'{"answerable":true,"answer":"test","support_ids":[],"confidence":0.8}'}
        self.assertTrue(score(r,self.row())['success']); self.assertFalse(score(r,self.row())['joint'])
    def test_incomplete_response_is_failure(self):
        self.assertFalse(score({'status':'incomplete','text':'{}'},self.row())['valid'])
    def test_unknown_support_id_is_invalid(self):
        r={'status':'completed','text':'{"answerable":true,"answer":"test","support_ids":[99],"confidence":0.8}'}
        self.assertFalse(score(r,self.row())['valid'])
    def test_complete_pairs_and_deterministic_selection(self):
        rows=[]
        for hop in (3,4):
            for i in range(9):
                for answerable in (False,True):
                    rows.append(dict(self.row(),id=f'{hop}hop__{i}',answerable=answerable))
        self.assertEqual(select(rows),select(list(reversed(rows))))
        self.assertEqual(len(select(rows)),24)
        with self.assertRaises(ValueError):select([r for r in rows if r['answerable']])

if __name__=='__main__':unittest.main()
