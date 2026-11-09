-- Synthetic OpenCode database (tables of OpenCode 1.2 and 2.2), loaded by the tests into
-- a WAL-mode `opencode.db`. Only the columns Veyra reads plus content columns holding
-- VEYRA-CANARY markers; no real data.

CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT NOT NULL, name TEXT,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,
  directory TEXT NOT NULL, title TEXT NOT NULL, version TEXT NOT NULL,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);
CREATE TABLE session_v2 (id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT,
  fork_session_id TEXT, directory TEXT NOT NULL, title TEXT, version TEXT NOT NULL,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
  type TEXT NOT NULL, seq INTEGER NOT NULL, data TEXT NOT NULL,
  time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
CREATE TABLE session_part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL,
  session_id TEXT NOT NULL, ordinal INTEGER NOT NULL, type TEXT NOT NULL, call_id TEXT,
  data TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);
CREATE TABLE event (id TEXT PRIMARY KEY, data TEXT NOT NULL);

INSERT INTO project VALUES ('prj_1', '/home/dev/api', 'VEYRA-CANARY project', 1794142800000, 1794142800000);
INSERT INTO event VALUES ('evt_1', '{"text":"VEYRA-CANARY event log"}');

-- 1.x session created by 1.3.5 (reasoning counted within output), later imported to 2.0.
INSERT INTO session VALUES ('ses_old', 'prj_1', NULL, '/home/dev/api', 'VEYRA-CANARY title', '1.3.5', 1794142800000, 1794142800000);
INSERT INTO message VALUES ('msg_u0', 'ses_old', 1794142800000, 1794142800000,
  '{"role":"user","time":{"created":1794142800000},"content":"VEYRA-CANARY prompt"}');
INSERT INTO message VALUES ('msg_old', 'ses_old', 1794142810000, 1794142820000,
  '{"role":"assistant","modelID":"claude-sonnet-4-5","providerID":"anthropic","time":{"created":1794142810000,"completed":1794142820000},"cost":0.01,"tokens":{"input":100,"output":60,"reasoning":20,"cache":{"read":1000,"write":50}},"path":{"cwd":"/home/dev/api","root":"/home/dev/api"}}');
INSERT INTO part VALUES ('prt_1', 'msg_old', 'ses_old', 1794142815000, 1794142815000,
  '{"type":"tool","callID":"call_a","tool":"bash","state":{"status":"completed","input":{"command":"VEYRA-CANARY"},"output":"VEYRA-CANARY"}}');
INSERT INTO part VALUES ('prt_2', 'msg_old', 'ses_old', 1794142815000, 1794142815000,
  '{"type":"text","text":"VEYRA-CANARY answer"}');

-- 1.x session created by 1.5.0 (reasoning apart from output), through GitHub Copilot.
INSERT INTO session VALUES ('ses_copilot', 'prj_1', NULL, '/home/dev/api', 'VEYRA-CANARY', '1.5.0', 1794143100000, 1794143100000);
INSERT INTO message VALUES ('msg_copilot', 'ses_copilot', 1794143110000, 1794143120000,
  '{"role":"assistant","modelID":"claude-sonnet-4.5","providerID":"github-copilot","time":{"created":1794143110000,"completed":1794143120000},"cost":0,"tokens":{"input":200,"output":30,"reasoning":10,"cache":{"read":0,"write":0}}}');

-- The 2.0 import of ses_old: same ids, reasoning now apart from output (same total).
INSERT INTO session_v2 VALUES ('ses_old', 'prj_1', NULL, NULL, '/home/dev/api', 'VEYRA-CANARY', '1.3.5', 1794142800000, 1794142800000);
INSERT INTO session_message VALUES ('msg_old', 'ses_old', 'assistant', 2,
  '{"agent":"build","model":{"id":"claude-sonnet-4-5","providerID":"anthropic"},"content":[{"type":"text","text":"VEYRA-CANARY"},{"type":"tool","id":"call_a","name":"bash","state":{"status":"completed","input":"VEYRA-CANARY","output":"VEYRA-CANARY"}}],"tokens":{"input":100,"output":40,"reasoning":20,"cache":{"read":1000,"write":50}},"time":{"created":1794142810000,"completed":1794142820000}}',
  1794142810000, 1794142820000);

-- A 2.2 session on Windows, with a compaction and a message still streaming.
INSERT INTO session_v2 VALUES ('ses_new', 'prj_1', NULL, NULL, 'C:/work/app', 'VEYRA-CANARY', '2.2.5', 1794146400000, 1794146400000);
INSERT INTO session_message VALUES ('msg_u1', 'ses_new', 'user', 1,
  '{"content":[{"type":"text","text":"VEYRA-CANARY prompt"}],"time":{"created":1794146400000}}', 1794146400000, 1794146400000);
INSERT INTO session_message VALUES ('msg_new', 'ses_new', 'assistant', 2,
  '{"agent":"build","model":{"id":"gpt-5.5","providerID":"openai"},"content":[{"type":"reasoning","text":"VEYRA-CANARY"},{"type":"tool","id":"call_b","name":"read","state":{"status":"completed","input":"VEYRA-CANARY","output":"VEYRA-CANARY"}}],"cost":0.02,"tokens":{"input":300,"output":80,"reasoning":40,"cache":{"read":2000,"write":0}},"time":{"created":1794146410000,"completed":1794146420000}}',
  1794146410000, 1794146420000);
INSERT INTO session_message VALUES ('msg_compact', 'ses_new', 'compaction', 3,
  '{"model":{"id":"gpt-5.5","providerID":"openai"},"summary":"VEYRA-CANARY summary","tokens":{"input":5000,"output":300,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1794146430000,"completed":1794146440000}}',
  1794146430000, 1794146440000);
INSERT INTO session_message VALUES ('msg_streaming', 'ses_new', 'assistant', 4,
  '{"model":{"id":"gpt-5.5","providerID":"openai"},"content":[{"type":"text","text":"VEYRA-CANARY"}],"time":{"created":1794146450000}}',
  1794146450000, 1794146450000);

-- A subagent of ses_new, with its tool call stored as a separate row.
INSERT INTO session_v2 VALUES ('ses_sub', 'prj_1', 'ses_new', NULL, 'C:/work/app', 'VEYRA-CANARY', '2.2.5', 1794146460000, 1794146460000);
INSERT INTO session_message VALUES ('msg_sub', 'ses_sub', 'assistant', 1,
  '{"agent":"explore","model":{"id":"gpt-5.5","providerID":"openai"},"content":[],"tokens":{"input":150,"output":20,"reasoning":0,"cache":{"read":0,"write":0}},"time":{"created":1794146470000,"completed":1794146480000}}',
  1794146470000, 1794146480000);
INSERT INTO session_part VALUES ('prt_s', 'msg_sub', 'ses_sub', 1, 'tool', 'call_c',
  '{"type":"tool","id":"call_c","name":"grep","state":{"status":"error","input":"VEYRA-CANARY","error":"VEYRA-CANARY"}}',
  1794146475000, 1794146475000);

-- A fork of ses_new made at 14:30: the copied message predates the fork and is skipped.
INSERT INTO session_v2 VALUES ('ses_fork', 'prj_1', NULL, 'ses_new', 'C:/work/app', 'VEYRA-CANARY', '2.2.5', 1794148200000, 1794148200000);
INSERT INTO session_message VALUES ('msg_fork_copy', 'ses_fork', 'assistant', 1,
  '{"model":{"id":"gpt-5.5","providerID":"openai"},"content":[],"tokens":{"input":300,"output":80,"reasoning":40,"cache":{"read":2000,"write":0}},"time":{"created":1794146410000,"completed":1794146420000}}',
  1794146410000, 1794148200000);
INSERT INTO session_message VALUES ('msg_fork_own', 'ses_fork', 'assistant', 2,
  '{"model":{"id":"gpt-5.5","providerID":"openai"},"content":[],"tokens":{"input":120,"output":10,"reasoning":0,"cache":{"read":500,"write":0}},"time":{"created":1794148210000,"completed":1794148220000}}',
  1794148210000, 1794148220000);
