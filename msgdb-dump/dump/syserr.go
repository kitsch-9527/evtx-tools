package dump

import (
	"strings"

	"golang.org/x/sys/windows"
)

// R15（2026-09-04）：Win32 系统错误码文本（%%2/%%8/%%21，如 SCM 7000/7023/7038）。
//
// wevtapi 展开参数值中的 %%N 引用时，先查 provider 的 ParameterMessageFile，
// 缺失则回退 FormatMessage(FORMAT_MESSAGE_FROM_SYSTEM)（kernel32/netmsg 系统消息）。
// SCM 注册项无 ParameterMessageFile（实测只有 EventMessageFile=services.exe），
// 故 %%2/%%8/%%21 在 parameter_refs 无行，输出残留 %21/%8。
//
// 本函数直调 Win32 FormatMessageW 取系统错误文本，调用方以 provider_name=
// "Win32-System-Error" 写入 parameter_refs（查询不分 provider，全局生效）。
func systemErrorText(code uint32, langID uint32) string {
	const fromSystem = 0x00001000 // FORMAT_MESSAGE_FROM_SYSTEM
	const maxChars = 2048
	buf := make([]uint16, maxChars)
	n, err := windows.FormatMessage(fromSystem, 0, code, langID, buf, nil)
	if err != nil || n == 0 {
		return ""
	}
	return strings.TrimSpace(windows.UTF16ToString(buf[:n]))
}

// collectSystemErrorMessages 扫描 Win32 错误码，返回有文本的 id→文本。
// 范围 0..maxCode（调用方定，通常 16000 覆盖 Win32+netmsg 常用段）。
func collectSystemErrorMessages(maxCode uint32, langID uint32) map[uint32]string {
	out := make(map[uint32]string)
	for code := uint32(0); code <= maxCode; code++ {
		if text := systemErrorText(code, langID); text != "" {
			out[code] = text
		}
	}
	return out
}
