package dump

import (
	"database/sql"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

// v9（2026-09-29）：参数引用按语言落库。此前 writeParamRefs 把所有行写成单一
// primaryCode（en-US），zh 渲染查不到本地化 %%N 文本（语言漂移校验事件，
// Security-Auditing 9733 条 D 的根因）。parameter_refs 的 PK 本就含 lang 列，
// 本测试钉住：Messages→primaryCode 层 + MessagesByLang→各自语言层。
func TestWriteParamRefs_MultiLang(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "param_refs.db")
	refs := []paramRefEntry{
		{
			ProviderName: "Microsoft-Windows-Security-Auditing",
			Messages:     map[uint32]string{1833: "Impersonation"},
			MessagesByLang: map[string]map[uint32]string{
				"zh-CN": {1833: "\u6a21\u62df", 8448: "\u5426"},
			},
		},
		{
			// 全局组（Win32 系统错误）只带主语言层，不得误写 ByLang。
			ProviderName: "Win32-System-Error",
			Messages:     map[uint32]string{2: "The system cannot find the file specified."},
		},
	}
	if err := writeParamRefs(dbPath, refs, "en-US"); err != nil {
		t.Fatalf("writeParamRefs: %v", err)
	}

	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	var msg string
	check := func(query, want string) {
		t.Helper()
		if err := db.QueryRow(query).Scan(&msg); err != nil {
			t.Fatalf("missing row (%s): %v", query, err)
		}
		if msg != want {
			t.Errorf("message = %q, want %q", msg, want)
		}
	}
	check("SELECT message FROM parameter_refs WHERE provider_name='Microsoft-Windows-Security-Auditing' AND param_id=1833 AND lang='en-US'", "Impersonation")
	check("SELECT message FROM parameter_refs WHERE provider_name='Microsoft-Windows-Security-Auditing' AND param_id=1833 AND lang='zh-CN'", "\u6a21\u62df")
	check("SELECT message FROM parameter_refs WHERE provider_name='Microsoft-Windows-Security-Auditing' AND param_id=8448 AND lang='zh-CN'", "\u5426")

	var n int
	if err := db.QueryRow("SELECT COUNT(*) FROM parameter_refs WHERE provider_name='Win32-System-Error'").Scan(&n); err != nil {
		t.Fatalf("count Win32: %v", err)
	}
	if n != 1 {
		t.Errorf("Win32 rows = %d, want 1 (only primary lang layer)", n)
	}
}

// v9：参数文件走 MUI 多语言萃取。msobjs.dll（Security-Auditing 的 ParameterMessageFile）
// 基础文件是 en，zh 文本在 zh-CN\msobjs.dll.mui——此前只读基础文件，%%1833 永远是
// "Impersonation"。本测试用本机真实文件做活体夹具（缺文件则跳过，与 syserr_test 同风格）。
func TestExtractParamMessagesByLang_MsobjsLive(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("windows only")
	}
	path := filepath.Join(os.Getenv("SystemRoot"), "System32", "msobjs.dll")
	if _, err := os.Stat(path); err != nil {
		t.Skipf("msobjs.dll not found: %v", err)
	}
	byLang := extractParamMessagesByLang(path, []string{"en-US", "zh-CN"})
	en := byLang["en-US"]
	zh := byLang["zh-CN"]
	if len(en) == 0 {
		t.Fatal("en-US param messages empty")
	}
	if len(zh) == 0 {
		t.Fatal("zh-CN param messages empty — MUI companion not extracted")
	}
	if strings.TrimSpace(en[1833]) != "Impersonation" {
		t.Errorf("en 1833 = %q, want Impersonation", en[1833])
	}
	if zh[1833] == "" || zh[1833] == en[1833] {
		t.Errorf("zh 1833 should be localized, got %q", zh[1833])
	}
}
