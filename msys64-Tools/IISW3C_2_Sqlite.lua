
--local myPath = arg[1]

--if myPath then
--    print("接收到的路径是：" .. myPath)
--else
--    print("请在运行脚本时传入一个路径参数！")
--end

--  加载 LuaFileSystem 库

local lfs = require("lfs")
local exist_TABLE = false
--os.execute("cls" )

-- 2. 获取当前目录路径
local current_dir = lfs.currentdir() .. "\\inetpub\\logs\\LogFiles"
--local current_dir = myPath .. "\\evtx"
print("正在遍历目录: " .. current_dir)
print("查找 *.log 文件...\n")


-- 3. 遍历目录
for filename in lfs.dir(current_dir) do
    -- 排除当前目录 "." 和上级目录 ".."
    if filename ~= "." and filename ~= ".." then
        -- 4. 使用 string.match 进行模式匹配
        -- 检查文件名是否以 ".evtx" 结尾
        if string.match(filename, "W3SVC") then
            print("找到文件: " .. current_dir .. "\\" .. filename .. "\\" .. "u_ex*.log")
			-- evtx转到文本 -e
			--local LogParserstr = string.format("LogParser -i:IISW3C  \"SELECT * INTO %s.CSV FROM inetpub\\logs\\LogFiles\\%s\\u_ex*.log\"", filename, filename)
            local LogParserstr = string.format("LogParser -e -i:IISW3C  \"SELECT * INTO %s.CSV FROM inetpub\\logs\\LogFiles\\%s\\u_ex*.log\"", filename, filename)
			os.execute(LogParserstr)

			if not exist_TABLE then
				local sqlite3str = string.format(("sqlite3.exe \"W3SVC.db\" \"PRAGMA synchronous=OFF; PRAGMA journal_mode=WAL; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-640000; BEGIN;\" \".mode csv\" \".import         %s.CSV W3SVC\" \"COMMIT;\""), filename)
                os.execute(sqlite3str)
				exist_TABLE = true
			else
                local sqlite3str1 =string.format(("sqlite3.exe \"W3SVC.db\" \"PRAGMA synchronous=OFF; PRAGMA journal_mode=WAL; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-640000; BEGIN;\" \".mode csv\" \".import --skip 1 %s.CSV W3SVC\" \"COMMIT;\""), filename)
                os.execute(sqlite3str1)
			end

			--os.execute(string.format("rm %s.CSV",filename))

        end
    end
end

os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'LogFilename'; CREATE TABLE 'LogFilename' AS SELECT \\\"LogFilename\\\",COUNT(*) FROM W3SVC GROUP BY \\\"LogFilename\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'LogRow'; CREATE TABLE 'LogRow' AS SELECT \\\"LogRow\\\",COUNT(*) FROM W3SVC GROUP BY \\\"LogRow\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'date'; CREATE TABLE 'date' AS SELECT \\\"date\\\",COUNT(*) FROM W3SVC GROUP BY \\\"date\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'time'; CREATE TABLE 'time' AS SELECT \\\"time\\\",COUNT(*) FROM W3SVC GROUP BY \\\"time\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'c-ip'; CREATE TABLE 'c-ip' AS SELECT \\\"c-ip\\\",COUNT(*) FROM W3SVC GROUP BY \\\"c-ip\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-username'; CREATE TABLE 'cs-username' AS SELECT \\\"cs-username\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-username\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-sitename'; CREATE TABLE 's-sitename' AS SELECT \\\"s-sitename\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-sitename\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-computername'; CREATE TABLE 's-computername' AS SELECT \\\"s-computername\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-computername\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-ip'; CREATE TABLE 's-ip' AS SELECT \\\"s-ip\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-ip\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-port'; CREATE TABLE 's-port' AS SELECT \\\"s-port\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-port\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-method'; CREATE TABLE 'cs-method' AS SELECT \\\"cs-method\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-method\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-uri-stem'; CREATE TABLE 'cs-uri-stem' AS SELECT \\\"cs-uri-stem\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-uri-stem\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-uri-query'; CREATE TABLE 'cs-uri-query' AS SELECT \\\"cs-uri-query\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-uri-query\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'sc-status'; CREATE TABLE 'sc-status' AS SELECT \\\"sc-status\\\",COUNT(*) FROM W3SVC GROUP BY \\\"sc-status\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'sc-substatus'; CREATE TABLE 'sc-substatus' AS SELECT \\\"sc-substatus\\\",COUNT(*) FROM W3SVC GROUP BY \\\"sc-substatus\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'sc-win32-status'; CREATE TABLE 'sc-win32-status' AS SELECT \\\"sc-win32-status\\\",COUNT(*) FROM W3SVC GROUP BY \\\"sc-win32-status\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'sc-bytes'; CREATE TABLE 'sc-bytes' AS SELECT \\\"sc-bytes\\\",COUNT(*) FROM W3SVC GROUP BY \\\"sc-bytes\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-bytes'; CREATE TABLE 'cs-bytes' AS SELECT \\\"cs-bytes\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-bytes\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'time-taken'; CREATE TABLE 'time-taken' AS SELECT \\\"time-taken\\\",COUNT(*) FROM W3SVC GROUP BY \\\"time-taken\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-version'; CREATE TABLE 'cs-version' AS SELECT \\\"cs-version\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-version\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs-host'; CREATE TABLE 'cs-host' AS SELECT \\\"cs-host\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs-host\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs(User-Agent)'; CREATE TABLE 'cs(User-Agent)' AS SELECT \\\"cs(User-Agent)\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs(User-Agent)\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs(Cookie)'; CREATE TABLE 'cs(Cookie)' AS SELECT \\\"cs(Cookie)\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs(Cookie)\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 'cs(Referer)'; CREATE TABLE 'cs(Referer)' AS SELECT \\\"cs(Referer)\\\",COUNT(*) FROM W3SVC GROUP BY \\\"cs(Referer)\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-event'; CREATE TABLE 's-event' AS SELECT \\\"s-event\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-event\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-process-type'; CREATE TABLE 's-process-type' AS SELECT \\\"s-process-type\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-process-type\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-user-time'; CREATE TABLE 's-user-time' AS SELECT \\\"s-user-time\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-user-time\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-kernel-time'; CREATE TABLE 's-kernel-time' AS SELECT \\\"s-kernel-time\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-kernel-time\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-page-faults'; CREATE TABLE 's-page-faults' AS SELECT \\\"s-page-faults\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-page-faults\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-total-procs'; CREATE TABLE 's-total-procs' AS SELECT \\\"s-total-procs\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-total-procs\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-active-procs'; CREATE TABLE 's-active-procs' AS SELECT \\\"s-active-procs\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-active-procs\\\" ORDER BY COUNT(*) DESC;\"")
os.execute("sqlite3.exe \"W3SVC.db\" \"DROP TABLE IF EXISTS 's-stopped-procs'; CREATE TABLE 's-stopped-procs' AS SELECT \\\"s-stopped-procs\\\",COUNT(*) FROM W3SVC GROUP BY \\\"s-stopped-procs\\\" ORDER BY COUNT(*) DESC;\"")

os.execute("PAUSE")
os.exit()