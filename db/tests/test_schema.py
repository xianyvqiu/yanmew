"""内存数据库设计验证，不访问实际业务数据。"""
import json
import re
import sqlite3
import unittest
from pathlib import Path
T = "2026-09-19T00:00:00Z"
class SchemaTests(unittest.TestCase):
    def setUp(self):
        self.c = sqlite3.connect(":memory:")
        self.c.execute("PRAGMA foreign_keys=ON")
        self.schema_text = (Path(__file__).resolve().parents[1]/"schema.sql").read_text(encoding="utf-8")
        self.c.executescript(self.schema_text)
        self.c.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(1,'测试','人设','人设')")
        for i in (1,2):
            self.c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(?,1,'测试',?,?)",(i,T,T))
        self.c.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,1,'away',?,?,?,?)",(T,T,T,T))
        self.c.commit()
    def tearDown(self):
        self.c.close()
    def ready(self):
        self.c.execute("UPDATE travels SET facts_state='ready',destination='湖边',activities_json=?,facts_version=1,facts_ready_at=?",(json.dumps([{"elapsed_minutes":0,"description":"抵达湖边"}],ensure_ascii=False),T))
    def returned(self):
        self.ready()
        self.c.execute("UPDATE travels SET status='returned',actual_return_at=?,gift_kind='text'",(T,))
    def gift(self):
        self.returned()
        snap=json.dumps({"schema_version":1,"kind":"text","content":"明信片","note":"纪念"},ensure_ascii=False)
        self.c.execute("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(1,1,'明信片','纪念',?)",(T,))
        self.c.execute("UPDATE travels SET gift_state='done',gift_snapshot_json=?",(snap,))
    def journal(self):
        self.gift()
        self.c.execute("INSERT INTO travel_journals(session_id,travel_id,title,content,created_at,updated_at) VALUES(1,1,'出行','游记',?,?)",(T,T))
        self.c.execute("UPDATE travels SET journal_state='done'")
        self.c.execute("INSERT INTO letters(session_id,travel_id,sender,content,letter_kind,created_at) VALUES(1,1,'ai','回来啦','return',?)",(T,))
        self.c.commit()
    def test_schema_integrity(self):
        self.assertEqual(self.c.execute("PRAGMA foreign_keys").fetchone()[0],1)
        self.assertEqual(self.c.execute("PRAGMA foreign_key_check").fetchall(),[])
        self.assertEqual(self.c.execute("PRAGMA integrity_check").fetchone()[0],'ok')
        self.c.execute("INSERT INTO long_memories_fts(long_memories_fts) VALUES('rebuild')")
    def test_every_table_column_has_inline_comment(self):
        table_names = re.findall(r"CREATE (?:VIRTUAL )?TABLE IF NOT EXISTS\s+(\w+)", self.schema_text)
        missing = []
        for table in table_names:
            match = re.search(
                r"CREATE (?:VIRTUAL )?TABLE IF NOT EXISTS\s+" + re.escape(table) + r"\b.*?\n\);",
                self.schema_text,
                re.S,
            )
            self.assertIsNotNone(match, table)
            for row in self.c.execute(f'PRAGMA table_info("{table}")'):
                column = row[1]
                line = re.search(r"^\s*" + re.escape(column) + r"\b[^\n]*", match.group(0), re.M)
                if line is None or "--" not in line.group(0) or not line.group(0).split("--", 1)[1].strip():
                    missing.append(f"{table}.{column}")
        self.assertEqual(missing, [], "缺少字段用途注释：" + ", ".join(missing))
    def test_every_enum_comment_lists_all_values(self):
        incomplete = []
        pattern = re.compile(r"^\s*(\w+)\b.*?CHECK\s*\(\s*\1\s+IN\s*\((.*?)\)\).*?--\s*(.+)$")
        for line in self.schema_text.splitlines():
            match = pattern.match(line)
            if not match:
                continue
            values = [quoted or number for quoted,number in re.findall(r"'([^']+)'|(?<!\w)(\d+)(?!\w)",match.group(2))]
            missing_values = [value for value in values if value not in match.group(3)]
            if missing_values:
                incomplete.append(f"{match.group(1)}: {','.join(missing_values)}")
        self.assertEqual(incomplete, [], "枚举字段注释没有列全取值：" + "; ".join(incomplete))
    def test_single_travel_countdown(self):
        columns={row[1] for row in self.c.execute("PRAGMA table_info(sessions)")}
        self.assertIn('travel_trigger_remaining_seconds',columns)
        self.assertNotIn('active_seconds_since_return',columns)
        self.assertNotIn('next_travel_after_seconds',columns)
        self.c.execute("UPDATE sessions SET travel_trigger_remaining_seconds=7200 WHERE id=1")
        with self.assertRaises(sqlite3.IntegrityError):
            self.c.execute("UPDATE sessions SET travel_trigger_remaining_seconds=0 WHERE id=1")
    def test_settings_key_is_not_nullable(self):
        key_column=next(row for row in self.c.execute("PRAGMA table_info(settings)") if row[1]=='key')
        self.assertEqual(key_column[3],1)
        with self.assertRaises(sqlite3.IntegrityError):
            self.c.execute("INSERT INTO settings(key,value) VALUES(NULL,'x')")
    def test_fact_shape(self):
        for a in (None,'bad','{}','[]'):
            with self.subTest(a=a),self.assertRaises(sqlite3.IntegrityError):
                self.c.execute("UPDATE travels SET facts_state='ready',destination='湖边',activities_json=?,facts_version=1,facts_ready_at=?",(a,T))
        self.ready()
    def test_terminal_state(self):
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE travels SET status='returned'")
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE travels SET status='cancelled',cancelled_at=?",(T,))
        self.c.execute("UPDATE travels SET status='cancelled',cancelled_at=?,facts_state='cancelled'",(T,))
    def test_cross_session(self):
        statements=[
          ("INSERT INTO travel_journals(session_id,travel_id,title,content,created_at,updated_at) VALUES(2,1,'a','b',?,?)",(T,T)),
          ("INSERT INTO letters(session_id,travel_id,sender,content,letter_kind,created_at) VALUES(2,1,'ai','a','midway',?)",(T,)),
          ("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(2,1,'a','b',?)",(T,)),
          ("INSERT INTO accessories(session_id,travel_id,name,display_text,description,source,created_at) VALUES(2,1,'a','b','c','travel',?)",(T,))]
        for sql,args in statements:
            with self.subTest(sql=sql),self.assertRaises(sqlite3.IntegrityError):self.c.execute(sql,args)
    def test_gift_snapshot(self):
        self.returned()
        for snap in ('bad','{}','[]','{"schema_version":1,"kind":"accessory"}','{"schema_version":2,"kind":"text"}'):
            with self.subTest(snap=snap),self.assertRaises(sqlite3.IntegrityError):
                self.c.execute("UPDATE travels SET gift_state='done',gift_snapshot_json=?",(snap,))
    def test_duplicate_gift(self):
        self.gift()
        with self.assertRaises(sqlite3.IntegrityError):
            self.c.execute("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(1,1,'a','b',?)",(T,))
    def test_delete_children(self):
        self.journal()
        before=self.c.execute("SELECT destination,activities_json,gift_snapshot_json FROM travels").fetchone()
        for table in ('travel_journals','letters','souvenirs'):self.c.execute('DELETE FROM '+table)
        self.assertEqual(self.c.execute("SELECT destination,activities_json,gift_snapshot_json FROM travels").fetchone(),before)
        self.assertEqual(self.c.execute("SELECT gift_state,journal_state FROM travels").fetchone(),('done','done'))
    def test_delete_parent(self):
        self.journal();self.c.execute("DELETE FROM travels")
        self.assertEqual(self.c.execute("SELECT count(*) FROM travel_journals").fetchone()[0],0)
        for table in ('letters','souvenirs'):
            self.assertEqual(self.c.execute('SELECT session_id,travel_id FROM '+table).fetchone(),(1,None))
        self.assertEqual(self.c.execute("PRAGMA foreign_key_check").fetchall(),[])
    def test_delete_session(self):
        self.journal();self.c.execute("DELETE FROM sessions WHERE id=1")
        for table in ('travels','travel_journals','letters','souvenirs'):
            self.assertEqual(self.c.execute('SELECT count(*) FROM '+table).fetchone()[0],0)
    def test_rollback(self):
        self.returned();self.c.commit()
        with self.assertRaises(sqlite3.IntegrityError):
            with self.c:
                self.c.execute("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(1,1,'a','b',?)",(T,))
                self.c.execute("UPDATE travels SET gift_state='done'")
        self.assertEqual(self.c.execute("SELECT count(*) FROM souvenirs").fetchone()[0],0)
        self.assertEqual(self.c.execute("SELECT gift_state FROM travels").fetchone()[0],'pending')
    def test_one_builtin_embedding(self):
        self.c.execute("INSERT INTO embedding_providers(name,kind,model,is_builtin) VALUES('内置','local','bge',1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.c.execute("INSERT INTO embedding_providers(name,kind,model,is_builtin) VALUES('重复','local','bge',1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.c.execute("INSERT INTO embedding_providers(name,kind,base_url,model,is_builtin) VALUES('错误','online','https://example.com','bge',1)")
    def test_ledger_plugin_identity(self):
        self.c.execute("INSERT INTO game_runs(id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,started_at) VALUES(1,'game.a','游戏','1',1,'running',?)",(T,))
        sql="INSERT INTO currency_ledger(delta,reason,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at) VALUES(1,'plugin_game',1,?,?,'胜利',?,?)"
        for pid,version in [('game.b','1'),('game.a','2')]:
            with self.assertRaises(sqlite3.IntegrityError):self.c.execute(sql,(pid,version,'request',T))
        self.c.execute(sql,('game.a','1','request',T))
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute(sql,('game.a','1','request',T))
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("DELETE FROM game_runs WHERE id=1")
    def test_vector_storage_size(self):
        self.c.execute("INSERT INTO chat_rounds(id,session_id,status,created_at) VALUES(1,1,'completed',?)",(T,))
        self.c.execute("INSERT INTO chat_rounds(id,session_id,status,created_at) VALUES(2,1,'completed',?)",(T,))
        self.c.execute("INSERT INTO messages(id,session_id,round_id,role,content,created_at) VALUES(1,1,1,'assistant','回复',?)",(T,))
        self.c.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(1,1,'标题','记忆',1,2,?,?)",(T,T))
        sql="INSERT INTO long_memory_embeddings(long_memory_id,session_id,model_key,dimensions,vector,content_hash,updated_at) VALUES(1,1,'model',2,?,'hash',?)"
        for value in (b'1234','12345678'):
            with self.assertRaises(sqlite3.IntegrityError):self.c.execute(sql,(value,T))
        self.c.execute(sql,(b'12345678',T))
    def test_long_memory_boundary_is_derived_from_sources(self):
        columns={row[1] for row in self.c.execute("PRAGMA table_info(long_memories)")}
        self.assertNotIn('covered_through_message_id',columns)
        for round_id in (1,2,3):
            self.c.execute("INSERT INTO chat_rounds(id,session_id,status,created_at) VALUES(?,1,'completed',?)",(round_id,T))
        for memory_id,round_id in ((1,1),(2,2)):
            self.c.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(?,1,?,'短期','integrated',?,?)",(memory_id,round_id,T,T))
        self.c.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(1,1,'标题','长期',2,3,?,?)",(T,T))
        self.c.execute("INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(1,1,1),(1,2,2)")
        boundary_round=self.c.execute("SELECT sm.round_id FROM long_memory_sources lms JOIN short_memories sm ON sm.id=lms.short_memory_id WHERE lms.long_memory_id=1 ORDER BY lms.source_order DESC LIMIT 1").fetchone()[0]
        self.assertEqual(boundary_round,2)
    def test_cross_session_update_rejected(self):
        self.journal()
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE letters SET session_id=2")
        with self.assertRaises(sqlite3.IntegrityError):self.c.execute("UPDATE travels SET session_id=2")

if __name__ == '__main__':unittest.main()
