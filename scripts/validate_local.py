#!/usr/bin/env python3
"""Exercise the real SQL schema and synthetic fixtures without a Rust compiler.

These checks do not execute Rust, compile dependencies, or measure extraction quality.
The optional Chromium check exercises the external helper, not the Rust adapter.
"""
from __future__ import annotations
import argparse
import csv
import functools
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import threading
import time
import tomllib
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = (ROOT / "migrations/001_initial.sql").read_text()

class SqlSchemaTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.executescript(SCHEMA)
    def tearDown(self):
        self.db.close()
    def doc(self, ident="a", text="source evidence", title="Title"):
        self.db.execute("INSERT INTO documents VALUES(?,?,?,?,?,?)",
                        (ident,title,"https://example.invalid", "2026-01-01",text,json.dumps({"id":ident})))
    def library(self, name="team"):
        self.db.execute("INSERT INTO libraries VALUES(?,?,?)", (name,"Shared","2026-01-01"))
    def results(self, query):
        return self.db.execute("SELECT rowid FROM documents_fts WHERE documents_fts MATCH ?", (query,)).fetchall()
    def test_schema_version(self):
        self.assertEqual(self.db.execute("PRAGMA user_version").fetchone()[0],1)
    def test_schema_is_idempotent(self):
        self.doc(); self.db.executescript(SCHEMA)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM documents").fetchone()[0],1)
    def test_fts_insert_trigger(self):
        self.doc(); self.assertEqual(len(self.results("evidence")),1)
    def test_fts_update_trigger(self):
        self.doc(); self.db.execute("UPDATE documents SET plain_text='replacement' WHERE id='a'")
        self.assertEqual(self.results("evidence"),[])
        self.assertEqual(len(self.results("replacement")),1)
    def test_fts_delete_trigger(self):
        self.doc(); self.db.execute("DELETE FROM documents WHERE id='a'")
        self.assertEqual(self.results("evidence"),[])
    def test_title_is_indexed(self):
        self.doc(title="UniqueHeading"); self.assertEqual(len(self.results("UniqueHeading")),1)
    def test_unicode_text_roundtrip(self):
        text="é漢字 🦀 0"; self.doc(text=text)
        self.assertEqual(self.db.execute("SELECT plain_text FROM documents").fetchone()[0],text)
    def test_one_document_in_two_libraries(self):
        self.doc()
        for name in ["alice","bob"]:
            self.library(name)
            self.db.execute("INSERT INTO library_items VALUES(?,?,?,?)",(name,"a",name,"2026-01-01"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM documents").fetchone()[0],1)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM library_items").fetchone()[0],2)
    def test_membership_is_idempotent(self):
        self.doc();self.library()
        sql="INSERT OR IGNORE INTO library_items VALUES('team','a','Alice','2026-01-01')"
        self.db.execute(sql);self.db.execute(sql)
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM library_items").fetchone()[0],1)
    def test_missing_document_cannot_be_added(self):
        self.library()
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO library_items VALUES('team','missing',NULL,'2026-01-01')")
    def test_document_json_constraint(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO documents VALUES('a','T','u','t','text','bad json')")
    def test_annotations_require_real_documents(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO annotations VALUES('n','missing','{}')")
    def test_annotation_roundtrip(self):
        self.doc();payload=json.dumps({"actor":"Alice","note":"Exact note","tags":["paper"]})
        self.db.execute("INSERT INTO annotations VALUES('n','a',?)",(payload,))
        self.assertEqual(json.loads(self.db.execute("SELECT payload FROM annotations").fetchone()[0])["note"],"Exact note")
    def test_cache_upsert(self):
        self.doc("a");self.doc("b")
        sql="INSERT INTO fetch_cache VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET document_id=excluded.document_id,checked_at=excluded.checked_at"
        self.db.execute(sql,("key","a",1));self.db.execute(sql,("key","b",2))
        self.assertEqual(self.db.execute("SELECT document_id,checked_at FROM fetch_cache").fetchone(),("b",2))
    def test_expired_cache_filter(self):
        self.doc();self.db.execute("INSERT INTO fetch_cache VALUES('key','a',1)")
        self.assertEqual(self.db.execute("SELECT * FROM fetch_cache WHERE checked_at>=2").fetchall(),[])
    def test_cascade_does_not_leave_library_references(self):
        self.doc();self.library();self.db.execute("INSERT INTO library_items VALUES('team','a',NULL,'t')")
        self.db.execute("DELETE FROM documents WHERE id='a'")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM library_items").fetchone()[0],0)
    def test_transaction_rollback(self):
        self.doc();self.db.commit();self.db.execute("BEGIN")
        self.db.execute("DELETE FROM documents");self.db.rollback()
        self.assertEqual(len(self.results("evidence")),1)
    def test_job_upsert(self):
        sql="INSERT INTO jobs VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at,payload=excluded.payload"
        self.db.execute(sql,("j","queued","t","t",'{"state":"queued"}'))
        self.db.execute(sql,("j","running","t","u",'{"state":"running"}'))
        self.assertEqual(self.db.execute("SELECT state,updated_at FROM jobs").fetchone(),("running","u"))
    def test_bad_job_json_is_rejected(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO jobs VALUES('j','queued','t','t','bad')")
    def test_fts_library_filter(self):
        self.doc("a");self.doc("b");self.library()
        self.db.execute("INSERT INTO library_items VALUES('team','a',NULL,'t')")
        rows=self.db.execute("SELECT d.id FROM documents_fts JOIN documents d ON d.rowid=documents_fts.rowid WHERE documents_fts MATCH ? AND EXISTS(SELECT 1 FROM library_items i WHERE i.document_id=d.id AND i.library=?)",('"evidence"','team')).fetchall()
        self.assertEqual(rows,[("a",)])
    def test_foreign_key_integrity(self):
        self.doc(); self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(),[])
    def test_database_integrity(self):
        self.doc(); self.assertEqual(self.db.execute("PRAGMA integrity_check").fetchone()[0],"ok")
    def test_disk_wal_and_two_connections(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/"store.sqlite3"
            a=sqlite3.connect(path);a.executescript(SCHEMA)
            self.assertEqual(a.execute("PRAGMA journal_mode=WAL").fetchone()[0],"wal")
            a.execute("INSERT INTO libraries VALUES('shared','','t')");a.commit()
            b=sqlite3.connect(path)
            self.assertEqual(b.execute("SELECT name FROM libraries").fetchone()[0],"shared")
            b.close();a.close()

class ArtifactChecks(unittest.TestCase):
    def test_cargo_manifests_are_valid_toml(self):
        for file in ROOT.rglob("Cargo.toml"):
            with file.open("rb") as stream: tomllib.load(stream)
    def test_workspace_members_exist(self):
        with (ROOT/"Cargo.toml").open("rb") as stream: config=tomllib.load(stream)
        for member in config["workspace"]["members"]:
            self.assertTrue((ROOT/member/"Cargo.toml").is_file())
    def test_client_does_not_depend_on_server_or_engine(self):
        with (ROOT/"crates/cli/Cargo.toml").open("rb") as stream: manifest=tomllib.load(stream)
        self.assertNotIn("webtool-engine",manifest["dependencies"])
        self.assertNotIn("webtool-server",manifest["dependencies"])
    def test_no_tui_dependency(self):
        with (ROOT/"Cargo.toml").open("rb") as stream: manifest=tomllib.load(stream)
        self.assertFalse({"ratatui","tui","cursive","termion"}&set(manifest["workspace"]["dependencies"]))
    def test_json_fixtures_are_valid(self):
        for name in ["data.json","notebook.ipynb"]:
            json.loads((ROOT/"tests/fixtures"/name).read_text())
    def test_jsonl_fixture_is_valid(self):
        for line in (ROOT/"tests/fixtures/data.jsonl").read_text().splitlines():json.loads(line)
    def test_xml_fixtures_are_valid(self):
        for name in ["feed.atom","feed.rss","paper.xml"]:ET.parse(ROOT/"tests/fixtures"/name)
    def test_csv_fixture_has_exact_edge_cases(self):
        with (ROOT/"tests/fixtures/values.csv").open(newline="") as stream: rows=list(csv.reader(stream))
        self.assertEqual(rows[1],["0","","=SUM(A1:A2)"])
        self.assertEqual(rows[2][0],"first\nsecond")
    def test_rust_schema_include_resolves(self):
        path=ROOT/"crates/engine/src/../../../migrations/001_initial.sql"
        self.assertEqual(path.read_text(),SCHEMA)
    def test_all_sources_are_utf8_without_nul(self):
        for path in ROOT.rglob("*.rs"):
            text=path.read_text(encoding="utf-8");self.assertNotIn("\0",text)

class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args): pass

def chromium_smoke() -> dict:
    chromium=shutil.which("chromium") or shutil.which("chromium-browser")
    if not chromium:return {"status":"not_run","reason":"Chromium not installed"}
    handler=functools.partial(QuietHandler,directory=str(ROOT/"tests/fixtures"))
    server=http.server.ThreadingHTTPServer(("127.0.0.1",0),handler)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="webtool-chromium-test-") as profile:
            command=[chromium,"--headless=new","--disable-gpu","--no-first-run","--no-default-browser-check",
                     "--disable-background-networking","--dump-dom","--virtual-time-budget=2000",f"--user-data-dir={profile}"]
            unsandboxed=hasattr(os,"geteuid") and os.geteuid()==0
            if unsandboxed:command.append("--no-sandbox")
            command.append(f"http://127.0.0.1:{server.server_port}/dynamic.html")
            started=time.monotonic()
            process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=os.name!="nt")
            try:
                stdout,stderr=process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                if os.name!="nt":os.killpg(process.pid,signal.SIGKILL)
                else:process.kill()
                stdout,stderr=process.communicate()
                return {"status":"failed","reason":"helper timed out","scope":"external Chromium only"}
            finally:
                if os.name!="nt":
                    try:os.killpg(process.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
            marker=b"<p>JS_RENDERED_EVIDENCE_17</p>" in stdout
            return {"status":"passed" if process.returncode==0 and marker else "failed",
                    "scope":"external Chromium CLI only, not Rust integration",
                    "exit_code":process.returncode,"dynamic_dom_marker_found":marker,
                    "root_test_used_no_sandbox":unsandboxed,
                    "elapsed_seconds":round(time.monotonic()-started,3),
                    "stderr_tail":stderr.decode(errors="replace")[-1200:],
                    "not_a_performance_benchmark":True}
    finally:
        server.shutdown();server.server_close()

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chromium",action="store_true")
    parser.add_argument("--report",type=Path,default=ROOT/"docs/local-validation.json")
    args=parser.parse_args()
    suite=unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in [SqlSchemaTests,ArtifactChecks]])
    stream=io.StringIO();result=unittest.TextTestRunner(stream=stream,verbosity=2).run(suite)
    print(stream.getvalue(),end="")
    helper=chromium_smoke() if args.chromium else {"status":"not_run"}
    report={"scope":"Real SQL migration and fixture checks, not Rust execution",
            "sqlite_version":sqlite3.sqlite_version,"tests_run":result.testsRun,
            "failures":len(result.failures),"errors":len(result.errors),"skipped":len(result.skipped),
            "rust_compilation":{"status":"not_run","reason":"This validation script does not invoke Cargo.","cargo_available":shutil.which("cargo") is not None},
            "rust_tests":{"status":"not_run"},"external_chromium":helper,
            "live_search":{"status":"not_run"},"xberg_and_ocr":{"status":"not_run"},
            "lightpanda":{"status":"not_run"},"yt_dlp":{"status":"not_run"}}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2)+"\n")
    args.report.with_suffix(".log").write_text(stream.getvalue())
    print(json.dumps(report,indent=2))
    return 0 if result.wasSuccessful() and helper["status"]!="failed" else 1
if __name__=="__main__":raise SystemExit(main())
