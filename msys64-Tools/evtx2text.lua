


-- 1. 加载 LuaFileSystem 库
local lfs = require("lfs")

os.execute("cls" )
-- 2. 获取当前目录路径
local current_dir = lfs.currentdir() .. "\\evtx"
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
			local evtx_filename = "evtx\\" .. filename
			-- evtx转到文本
			local wevtutilstr = string.format("wevtutil qe \"%s\" /f:text /lf:true | tr '\\r\\n' ';' |sed 's/\\x00/:/g' | perl -pe \"s/Event\\[/\\x0D\\x0A$&/g\" |iconv -f GBK -t UTF-8 > \"%s.txt\"", evtx_filename, filename)
			os.execute(wevtutilstr)

			--local sqlstr = string.format("LogParser -i:EVT \"SELECT 'Data__Start',* INTO '%s.CSV' FROM '%s'\"", filename, evtx_filename)
			--os.execute(sqlstr) -- LogParser -i:EVT "SELECT 'Data__Start',* INTO Application.CSV FROM Application.evtx"
			--local awkstr = string.format("awk -F',' \"{print $2}\" 'TimeGenerated_EventID_Message_%s.CSV' |tail -n +2 | sort | uniq -c >\"EventID_sort_uniq_%s.txt\"", filename, filename)
			--os.execute(awkstr)
			--local awk1str = string.format("awk \"{print $2}\" \"EventID_sort_uniq_%s.txt\" |  tr -d '\r' | xargs -P 7 -I {} bash -c \"grep {} 'TimeGenerated_EventID_Message_%s.CSV' > EventID-{}.CSV\"", filename, filename)
			--os.execute(awk1str)
        end
    end
end
os.execute("PAUSE")
