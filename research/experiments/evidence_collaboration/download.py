"""Fetch and CRC-check the official MuSiQue archive, using bounded range requests."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import time
import urllib.request
import zipfile

URL = 'https://drive.usercontent.google.com/download?id=1tGdADlNjWFaHLeZZGShh2IRcpO6Lv24h&export=download&confirm=t'
SIZE = 272049578  # Observed official Content-Range total, checked on every chunk.
CHUNK = 2 * 1024 * 1024
ENTRIES = ['data/musique_full_v1.0_train.jsonl', 'data/musique_full_v1.0_dev.jsonl']


def download(folder):
    folder.mkdir(parents=True, exist_ok=True)
    chunks = folder / 'chunks'
    chunks.mkdir(exist_ok=True)
    def fetch(start):
        end = min(start + CHUNK, SIZE) - 1
        dest = chunks / str(start)
        if dest.exists() and dest.stat().st_size == end - start + 1:
            return
        for attempt in range(3):
            try:
                req = urllib.request.Request(URL, headers={'Range':f'bytes={start}-{end}'})
                with urllib.request.urlopen(req, timeout=120) as response:
                    if response.status != 206 or response.headers.get('Content-Range') != f'bytes {start}-{end}/{SIZE}':
                        raise ValueError('Unexpected archive range response')
                    data = response.read(end-start+2)
                if len(data) != end-start+1:
                    raise ValueError('Truncated archive range')
                temporary = dest.with_suffix('.tmp')
                temporary.write_bytes(data)
                temporary.replace(dest)
                return
            except Exception:
                if attempt == 2:
                    raise
                time.sleep(1 + attempt)
    starts = list(range(0, SIZE, CHUNK))
    with ThreadPoolExecutor(max_workers=12) as pool:
        for i, _ in enumerate(pool.map(fetch, starts), 1):
            if i % 10 == 0:
                print(json.dumps({'chunks_ready':i,'total_chunks':len(starts)}), flush=True)
    archive = folder / 'musique_v1.0.zip'
    with archive.open('wb') as output:
        for start in starts:
            output.write((chunks/str(start)).read_bytes())
    with archive.open('rb') as f:
        sha=hashlib.file_digest(f,'sha256').hexdigest()
    manifest = {'url':URL, 'size':SIZE, 'sha256':sha, 'files':{}}
    with zipfile.ZipFile(archive) as z:
        bad = z.testzip()
        if bad:
            raise ValueError('CRC failure: '+bad)
        for name in ENTRIES:
            data = z.read(name)
            dest = folder / Path(name).name
            dest.write_bytes(data)
            manifest['files'][dest.name] = {'sha256':hashlib.sha256(data).hexdigest(), 'bytes':len(data)}
    (folder/'source.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest), flush=True)

if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('folder',type=Path)
    download(p.parse_args().folder)
