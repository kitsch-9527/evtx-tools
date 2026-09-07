package mui

// LangMap maps known language codes to their LCID string representations.
var LangMap = map[string]string{
	"en-US": "1033",
	"zh-CN": "2052",
	"zh-TW": "1028",
	"ja-JP": "1041",
	"ko-KR": "1042",
}

// lcidToLang maps LCID -> language code.
var lcidToLang = func() map[string]string {
	m := make(map[string]string, len(LangMap))
	for code, lcid := range LangMap {
		m[lcid] = code
	}
	return m
}()

// LCIDToLangCode maps an LCID string to a language code.
func LCIDToLangCode(lcid string) string {
	if code, ok := lcidToLang[lcid]; ok {
		return code
	}
	return ""
}
