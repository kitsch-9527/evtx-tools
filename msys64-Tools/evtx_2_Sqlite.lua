
os.execute("chcp 65001 >nul" )
local myPath = arg[1]


if myPath then
    print("接收到的路径是：" .. myPath)
else
    print("请在运行脚本时传入一个路径参数！")
end

-- 1. 加载 LuaFileSystem 库
local lfs = require("lfs")
local exist_TABLE = false
os.execute("cls" )
-- 2. 获取当前目录路径
--local current_dir = lfs.currentdir() .. "\\evtx"

local current_dir = myPath 
print("正在遍历目录: " .. current_dir)
print("查找 *.evtx 文件...\n")

-- 3. 遍历目录
for filename in lfs.dir(current_dir) do
    
    -- 排除当前目录 "." 和上级目录 ".."
    if filename ~= "." and filename ~= ".." then
        -- 4. 使用 string.match 进行模式匹配
        -- 检查文件名是否以 ".evtx" 结尾
        if string.match(filename, "%.evtx$") then
            print("找到文件: " .. filename)
			-- evtx转到文本
			local evtx2csvstr = string.format("evtx2csv.exe \"evtx\\%s\" *", filename)
			os.execute(evtx2csvstr)

			if not exist_TABLE then 
            -- -- 1. 基础存储与内存配置
            -- PRAGMA page_size = 65536 ;        -- 64KB 最大页面（需配合 VACUUM 生效）
            -- PRAGMA cache_size = -786432;     -- 768MB 内存缓存
            -- PRAGMA temp_store = MEMORY;      -- 临时表强制内存
            -- 
            -- -- 2. 极致写入性能配置
            -- PRAGMA journal_mode = OFF;       -- 预写日志
            -- PRAGMA synchronous = OFF;        -- 关闭同步（最危险但也最快）
            -- PRAGMA locking_mode = EXCLUSIVE; -- 独占锁
            -- -- 3. 约束检查关闭
            -- PRAGMA foreign_keys = OFF;
            ---- 2. 极致查询配置（手动查询的隐藏外挂）
            -- PRAGMA mmap_size = 2147483648;    -- 开启 256MB 内存映射，手动查询大表时速度起飞
            -- PRAGMA max_page_count=1000000; -- 限制最大页数为100万页 
            -- PRAGMA page_size=65536;  设置 256 MB PRAGMA  
 
				local sqlite3str = string.format(("sqlite3.exe \"evtx.db\" \"PRAGMA journal_mode = OFF; PRAGMA mmap_size = 2147483648;PRAGMA foreign_keys = OFF;PRAGMA max_page_count=1000000; PRAGMA page_size=65536 ; PRAGMA locking_mode = EXCLUSIVE; PRAGMA synchronous=OFF;  PRAGMA temp_store=MEMORY; PRAGMA cache_size=-786432; BEGIN;\" \".mode csv\" \".import         \\\"evtx\\\\%s.CSV\\\" evtx\" \"COMMIT;\""), filename)
                os.execute(sqlite3str)
				exist_TABLE = true
			else
                local sqlite3str1 =string.format(("sqlite3.exe \"evtx.db\" \"PRAGMA journal_mode = OFF; PRAGMA mmap_size = 2147483648;PRAGMA foreign_keys = OFF;PRAGMA max_page_count=1000000; PRAGMA page_size=-262144; PRAGMA locking_mode = EXCLUSIVE; PRAGMA synchronous=OFF;PRAGMA temp_store=MEMORY; PRAGMA cache_size=-786432; BEGIN;\" \".mode csv\" \".import --skip 1 \\\"evtx\\\\%s.CSV\\\" evtx\" \"COMMIT;\""), filename)
                os.execute(sqlite3str1)
			end
            os.execute("chcp 65001 >nul" )
        end
    end
end

os.execute("echo ATTACH DATABASE 'evtx.db' AS source_db;  >evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx AS SELECT EventID, TimeCreated, Message, Provider, Channel FROM source_db.evtx; >>evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx_AnyDesk; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx_AnyDesk AS SELECT * FROM aevtx  WHERE Message LIKE '%AnyDesk%'; >>evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx_4624_4625; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx_4624_4625 AS SELECT * FROM aevtx  WHERE  EventID IN (4624,4625); >>evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx_7045; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx_7045 AS SELECT * FROM aevtx  WHERE  EventID IN (7045); >>evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx_ASPNET; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx_ASPNET AS SELECT * FROM aevtx  WHERE  Provider LIKE 'ASP.NET%'; >>evtx_2_Sqlite.sql")
os.execute("echo DROP TABLE IF EXISTS aevtx_MSSQL_login; >>evtx_2_Sqlite.sql")
os.execute("echo CREATE TABLE aevtx_MSSQL_login AS SELECT * FROM aevtx  WHERE Provider LIKE 'MSSQL%' and Message LIKE '%登录%'; >>evtx_2_Sqlite.sql")
os.execute("echo .exit >>evtx_2_Sqlite.sql")
os.execute("sqlite3.exe evtx_new.db <evtx_2_Sqlite.sql")
os.execute("DEL evtx_2_Sqlite.sql")

os.execute("PAUSE")
os.exit()

