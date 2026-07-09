-- Layer 1: 规范化视图 (对应 Lua 的 aevtx)
DROP TABLE IF EXISTS aevtx;
CREATE TABLE aevtx AS
SELECT event_id, timestamp, message, provider_name, channel, severity, computer, user_name
FROM events;

-- Layer 2: 场景分析表 (对应 Lua 的 6 个场景)
-- 场景 1: 登录事件 (4624, 4625)
DROP TABLE IF EXISTS aevtx_4624_4625;
CREATE TABLE aevtx_4624_4625 AS
SELECT * FROM events WHERE event_id IN (4624, 4625);

-- 场景 2: 服务安装 (7045)
DROP TABLE IF EXISTS aevtx_7045;
CREATE TABLE aevtx_7045 AS
SELECT * FROM events WHERE event_id = 7045;

-- 场景 3: AnyDesk 远程桌面
DROP TABLE IF EXISTS aevtx_AnyDesk;
CREATE TABLE aevtx_AnyDesk AS
SELECT * FROM aevtx WHERE message LIKE '%AnyDesk%' OR message LIKE '%anydesk%';

-- 场景 4: ASP.NET 事件
DROP TABLE IF EXISTS aevtx_ASPNET;
CREATE TABLE aevtx_ASPNET AS
SELECT * FROM aevtx WHERE provider_name LIKE 'ASP.NET%';

-- 场景 5: MSSQL 登录
DROP TABLE IF EXISTS aevtx_MSSQL_login;
CREATE TABLE aevtx_MSSQL_login AS
SELECT * FROM aevtx WHERE provider_name LIKE 'MSSQL%' AND message LIKE '%登录%';

-- 统计表
DROP TABLE IF EXISTS stat_event_id;
CREATE TABLE stat_event_id AS
SELECT event_id, provider_name, COUNT(*) AS count
FROM events
GROUP BY event_id, provider_name
ORDER BY count DESC;

DROP TABLE IF EXISTS stat_severity;
CREATE TABLE stat_severity AS
SELECT level, severity, COUNT(*) AS count
FROM events GROUP BY level, severity
ORDER BY count DESC;

DROP TABLE IF EXISTS stat_hourly;
CREATE TABLE stat_hourly AS
SELECT substr(timestamp, 1, 13) AS hour_bucket, COUNT(*) AS count
FROM events WHERE timestamp IS NOT NULL
GROUP BY hour_bucket
ORDER BY hour_bucket;

DROP TABLE IF EXISTS stat_channel;
CREATE TABLE stat_channel AS
SELECT channel, COUNT(*) AS count
FROM events GROUP BY channel
ORDER BY count DESC;

DROP TABLE IF EXISTS stat_computer;
CREATE TABLE stat_computer AS
SELECT computer, COUNT(*) AS count
FROM events WHERE computer IS NOT NULL
GROUP BY computer
ORDER BY count DESC;

ANALYZE;
