use strict;
use warnings;
# use File::Basename;
use File::Spec;

my $dir       = $ARGV[0] || '.';
my $outdir    = $ARGV[1] || $dir;
my $evtx2csv = 'evtx2csv';
my $exist_TABLE = 0;

opendir(my $dh, $dir) or die "无法打开目录 $dir: $!";
my @evtx = sort grep { /\.evtx$/i } readdir($dh);
closedir($dh);

if (!@evtx) {
    print "目录 $dir 中未找到 .EVTX 文件\n";
    exit 0;
}

print "共找到 " . scalar(@evtx) . " 个 EVTX 文件\n";
print "=" x 50 . "\n";

my $sql_begin = <<"SQL";
.mode off
PRAGMA journal_mode = OFF;
PRAGMA mmap_size = 2147483648;
PRAGMA foreign_keys = OFF;
PRAGMA max_page_count = 1000000;
PRAGMA page_size = 65536;
PRAGMA locking_mode = EXCLUSIVE;
PRAGMA synchronous = OFF;
PRAGMA temp_store = MEMORY;
PRAGMA cache_size = -786432;
SQL

my $sqlite_cmd = qq("sqlite3.exe" "evtx.db");
open(my $fh, "|-", $sqlite_cmd) or die "Cannot run sqlite3: $!";
print $fh $sql_begin;


for my $file (@evtx) {
    my $fullpath = File::Spec->catfile($dir, $file);
    my $base     = $file;
    $base =~ s/\.evtx$//i;

    print "正在处理: $file\n";

    my $cmd = qq("$evtx2csv" "$fullpath" "*");

    print "  命令: $cmd\n";
    system($cmd);

    if ($? == 0) {
        print "  evtx2csv 完成: $fullpath\n";
    } else {
        print "  evtx2csv 失败: $file (退出码: " . ($? >> 8) . ")\n";
    }

    my $csv_file = "$fullpath.CSV";

    if ($exist_TABLE == 0) {
        # 通过管道传递 SQL，彻底告别命令行引号地狱
        my $sql_end = <<"SQL";
BEGIN;
.mode csv
.import "$csv_file" evtx
COMMIT;
SQL

        print $fh $sql_end;
        print "sqlite 导入数据完成\n";
        $exist_TABLE = 1
    } else {

        my $sql_skip_end = <<"SQL";
BEGIN;
.mode csv
.import --skip 1 "$csv_file" evtx
COMMIT;
SQL
        print $fh $sql_skip_end;
        print "sqlite 导入数据完成\n";
        
    }
    print "-" x 50 . "\n";
}

close($fh) or die "sqlite3 failed: $?";

my $sql_evtx_new = <<"SQL";
ATTACH DATABASE 'evtx.db' AS source_db; 
DROP TABLE IF EXISTS aevtx; 
CREATE TABLE aevtx AS SELECT EventID, TimeCreated, Message, Provider, Channel FROM source_db.evtx; 
DROP TABLE IF EXISTS aevtx_AnyDesk; 
CREATE TABLE aevtx_AnyDesk AS SELECT * FROM aevtx  WHERE Message LIKE '%AnyDesk%'; 
DROP TABLE IF EXISTS aevtx_4624_4625; 
CREATE TABLE aevtx_4624_4625 AS SELECT * FROM aevtx  WHERE  EventID IN (4624,4625); 
DROP TABLE IF EXISTS aevtx_7045; 
CREATE TABLE aevtx_7045 AS SELECT * FROM aevtx  WHERE  EventID IN (7045); 
DROP TABLE IF EXISTS aevtx_ASPNET; 
CREATE TABLE aevtx_ASPNET AS SELECT * FROM aevtx  WHERE  Provider LIKE 'ASP.NET%'; 
DROP TABLE IF EXISTS aevtx_MSSQL_login; 
CREATE TABLE aevtx_MSSQL_login AS SELECT * FROM aevtx  WHERE Provider LIKE 'MSSQL%' and Message LIKE '%登录%'; 
.exit 
SQL

$sqlite_cmd = qq("sqlite3.exe" "evtx_new.db");
open($fh, "|-", $sqlite_cmd) or die "Cannot run sqlite3: $!";
print $fh $sql_evtx_new;
close($fh) or die "sqlite3 failed: $?";

print "全部处理完毕\n";







