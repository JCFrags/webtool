#!/usr/bin/env python3
"""Run the built client and server against local fixtures. Requires real Rust binaries."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
def main()->int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir",type=Path,default=ROOT/"target/debug")
    args=parser.parse_args()
    suffix=".exe" if os.name=="nt" else ""
    client=(args.bin_dir/f"webtool{suffix}").resolve()
    server=(args.bin_dir/f"webtoold{suffix}").resolve()
    if not client.is_file() or not server.is_file():
        parser.exit(2,"Build both binaries first: cargo build -p webtool-cli -p webtool-server\n")
    with socket.socket() as sock:
        sock.bind(("127.0.0.1",0));port=sock.getsockname()[1]
    with tempfile.TemporaryDirectory(prefix="webtool-cli-test-") as directory:
        base=f"http://127.0.0.1:{port}"
        logpath=Path(directory)/"server.log"
        with logpath.open("wb") as log:
            process=subprocess.Popen([str(server),"--bind",f"127.0.0.1:{port}","--data-dir",directory],stdout=log,stderr=log)
            try:
                for _ in range(100):
                    if process.poll() is not None:raise RuntimeError(logpath.read_text())
                    try:
                        with urllib.request.urlopen(base+"/v1/health",timeout=1) as response:
                            if response.status==200:break
                    except OSError:time.sleep(0.1)
                else:raise RuntimeError("Server did not become ready")
                def run(*arguments:str):
                    p=subprocess.run([str(client),"--server",base,"--format","json",*arguments],capture_output=True,text=True,timeout=30)
                    if p.returncode:raise RuntimeError(f"{arguments}: {p.stderr}")
                    return json.loads(p.stdout) if p.stdout.strip() else None
                run("doctor")
                run("library","create","shared")
                document=run("ingest",str(ROOT/"tests/fixtures/source.md"),"--library","shared","--actor","Alice")
                ident=document["id"]
                assert run("read",ident)["id"]==ident
                assert run("find",ident,"Exact code")["matches"]
                assert run("extract",ident,"code")["data"]
                assert run("search","Exact code","--library","shared")["results"]
                run("note",ident,"--actor","Bob","--text","Reviewed")
                assert run("notes",ident)[0]["actor"]=="Bob"
                output=Path(directory)/"source-original.md"
                run("export",ident,"--kind","original","--output",str(output))
                assert output.read_bytes()==(ROOT/"tests/fixtures/source.md").read_bytes()
                table=run("ingest",str(ROOT/"tests/fixtures/values.csv"))
                run("export",table["id"],"--kind","table-csv","--output",str(Path(directory)/"table.csv"))
                print("PASS: actual CLI/server upload, shared library, read, find, extraction, local search, notes, and exports.")
                return 0
            finally:
                process.terminate()
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:process.kill();process.wait()
if __name__=="__main__":raise SystemExit(main())
