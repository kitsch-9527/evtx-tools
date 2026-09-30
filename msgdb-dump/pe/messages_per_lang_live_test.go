package pe

import (
	"strings"
	"testing"
)

// R27（2026-09-30）活体测试：e1dmsg.dll（DriverStore）单文件内嵌多语言
// RT_MESSAGETABLE（0x409/0x0804/0x0C0A…）。此前 ExtractAllFromPE 把所有语言块
// last-write-wins 合并进 Messages，西语块胜出并被存到 en-US/zh-CN 键下
// （e1dexpress 32×8 假阳性根因）。MessagesPerLang 必须按资源语言 ID 分取。
func TestLiveMessagesPerLangE1d(t *testing.T) {
	const p = `C:\Windows\System32\DriverStore\FileRepository\e1d68x64.inf_amd64_b32d0891eb98aee4\e1dmsg.dll`
	combined, err := ExtractAllFromPE(p)
	if err != nil {
		t.Fatalf("ExtractAllFromPE: %v", err)
	}
	if len(combined.MessagesPerLang) == 0 {
		t.Fatal("MessagesPerLang empty — per-language message tables not collected")
	}
	en := combined.MessagesPerLang[0x0409]
	zh := combined.MessagesPerLang[0x0804]
	es := combined.MessagesPerLang[0x0C0A]
	t.Logf("langs collected: %d tables (en=%d zh=%d es=%d)",
		len(combined.MessagesPerLang), len(en), len(zh), len(es))
	if len(zh) == 0 {
		t.Fatal("zh-CN (0x0804) table missing from MessagesPerLang")
	}
	foundZh := false
	for _, m := range zh {
		if strings.Contains(m, "网络链接已以") {
			foundZh = true
		}
	}
	if !foundZh {
		t.Error("zh table lacks the baseline network-link text")
	}
	if len(es) > 0 {
		for _, m := range es {
			if strings.Contains(m, "Se ha establecido") {
				return // es block intact under its own language key
			}
		}
	}
	// es 允许缺失（无所谓），但 zh 必须独立成立——上面已断言。
}
