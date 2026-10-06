"""Independent fixture checks run against each exact submitted bundle, after trials.
Not a complete specification proof; counted formation outcomes are separate.
"""
import importlib.util
from pathlib import Path
import sys

def check(name,root):
    spec=importlib.util.spec_from_file_location('candidate',Path(root)/(name+'.py'))
    m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
    if name=='slug':
        for text,want in [(' A_B--C ','a-b-c'),('Éclair déjà','clair-d-j'),('A東京B','a-b'),('---',''),('',''),('ABC123','abc123')]:assert m.slugify(text)==want
    elif name=='intervals':
        x=[[5,7],[1,3],[3,5],[9,10]];assert m.merge_intervals(x)==[[1,7],[9,10]];assert x==[[5,7],[1,3],[3,5],[9,10]]
        assert m.merge_intervals([])==[];assert m.merge_intervals([[0,0],[-1,0]])==[[-1,0]]
        try:m.merge_intervals([[3,2]])
        except ValueError:pass
        else:raise AssertionError('reversed interval accepted')
    elif name=='chunks':
        x=[1,2,3,4,5];assert m.chunks(x,2)==[[1,2],[3,4],[5]];assert x==[1,2,3,4,5];assert m.chunks([],2)==[]
        for n in [0,-1,True,False,1.5,'2']:
            try:m.chunks([1],n)
            except ValueError:pass
            else:raise AssertionError('invalid size accepted')
    elif name=='counts':
        assert m.word_counts("Hi hi! Can't 123 HI déjà")=={'hi':3,'can':1,'t':1,'d':1,'j':1}
        assert m.word_counts('')=={};assert m.word_counts('one2two ONE')=={'one':2,'two':1}
    elif name=='duration':
        for s,n in [('1h30m',5400),('2s 3m 1h',3782),('0s',0),('1m2h3s',7263)]:assert m.parse_duration(s)==n
        for s in ['', ' ', '1x', '-1s', '+1s', '1.5h', '1m2m','1h junk','1h30','1s1S']:
            try:m.parse_duration(s)
            except ValueError:pass
            else:raise AssertionError('invalid duration accepted: '+s)
    elif name=='unique':
        x=[[1],[1],{'a':2},{'a':2},[2]];assert m.stable_unique(x)==[[1],{'a':2},[2]];assert len(x)==5
        assert m.stable_unique([])==[];assert m.stable_unique([3,1,3,2])==[3,1,2]
    elif name=='rotate':
        x=[1,2,3];assert m.rotate(x,1)==[3,1,2];assert m.rotate(x,-1)==[2,3,1];assert m.rotate(x,100)==[3,1,2];assert m.rotate([],3)==[];assert m.rotate(x,0)==x;assert m.rotate(x,0) is not x;assert x==[1,2,3]
    elif name=='flatten':
        x=[1,[2,[],[3]],(4,5),'ab',{'x':1}];assert m.flatten(x)==[1,2,3,(4,5),'ab',{'x':1}];assert x==[1,[2,[],[3]],(4,5),'ab',{'x':1}];assert m.flatten([])==[]
    else:raise ValueError(name)
if __name__=='__main__':
    check(sys.argv[1],sys.argv[2]);print('oracle passed:',sys.argv[1])
