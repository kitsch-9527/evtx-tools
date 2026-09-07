package dump

import (
	"os"
	"path/filepath"

	"github.com/kitsch-9527/wcorefx/fs"
)

// LangMap maps known language codes to their LCID string representations.
// Used to convert between --lang CLI arguments (LCIDs) and MUI directory names.
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

// FindMUICompanion finds the MUI companion file for a DLL + language code.
//
// Windows stores localized message text in MUI companion files at:
//
//	{dll_dir}\{lang}\{dll_name}.dll.mui
//
// Uses wcorefx/fs.NormalizePath to handle WoW64 file-system redirection:
// C:\Windows\System32\foo.dll is remapped to C:\Windows\Sysnative\foo.dll
// when running as a 32-bit process on 64-bit Windows.
//
// Returns nil if no .mui file exists at the expected path.
func FindMUICompanion(dllPath, langCode string) *string {
	dll := fs.NormalizePath(dllPath)
	parent := filepath.Dir(dll)
	fileName := filepath.Base(dll)
	muiName := fileName + ".mui"
	muiPath := filepath.Join(parent, langCode, muiName)

	if _, err := os.Stat(muiPath); err == nil {
		return &muiPath
	}
	return nil
}

// MUICompanion is a single MUI companion file result.
type MUICompanion struct {
	Lang string // language code (e.g. "en-US")
	Path string // full path to the .mui file
}

// FindAllMUICompanions returns all available MUI companions for a DLL.
func FindAllMUICompanions(dllPath string) []MUICompanion {
	var result []MUICompanion
	for lang := range LangMap {
		if p := FindMUICompanion(dllPath, lang); p != nil {
			result = append(result, MUICompanion{Lang: lang, Path: *p})
		}
	}
	return result
}

// LCIDToLangCode maps an LCID string to a language code.
// "1033" -> "en-US". Falls back to "en-US" for unknown LCIDs.
func LCIDToLangCode(lcid string) string {
	if code, ok := lcidToLang[lcid]; ok {
		return code
	}
	return "en-US"
}

// LangCodeToLCID maps a language code to its LCID string.
// "en-US" -> "1033". Falls back to "1033" for unknown codes.
func LangCodeToLCID(langCode string) string {
	if lcid, ok := LangMap[langCode]; ok {
		return lcid
	}
	return "1033"
}
