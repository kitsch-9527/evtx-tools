package pe

import (
	"testing"
)

func TestParseWEVTTemplate_utf16leNoBom(t *testing.T) {
	// UTF-16LE without BOM — the first byte is '<' (0x3C) followed by NUL.
	// The old code would treat it as UTF-8 (because it starts with '<')
	// and fail to decode the XML properly.
	xml := `<template>
  <event value="100">
    <data name="Param1"/>
  </event>
</template>`
	utf16Data := utf16le(xml) // no BOM prefix

	result := ParseWEVTTemplate(utf16Data)
	if result == nil {
		t.Fatal("ParseWEVTTemplate(UTF16LE no BOM) = nil, want parsed result")
	}
	e100, ok := result["100"]
	if !ok {
		t.Fatal("expected event 100")
	}
	if e100["1"] != "Param1" {
		t.Fatalf(`e100["1"] = %q, want "Param1"`, e100["1"])
	}
}

func TestParseWEVTTemplate_mixedNulProtection(t *testing.T) {
	// Pure ASCII content containing "NUL" as literal text (not byte 0x00)
	// should not be falsely detected as UTF-16LE
	xml := "<template>\n  <event value=\"1\">\n    <data name=\"NUL\"/>\n  </event>\n</template>"
	result := ParseWEVTTemplate([]byte(xml))
	if result == nil {
		t.Fatal("ParseWEVTTemplate(ASCII with 'NUL' text) = nil, want parsed result")
	}
	e1, ok := result["1"]
	if !ok {
		t.Fatal("expected event 1")
	}
	if e1["1"] != "NUL" {
		t.Fatalf(`e1["1"] = %q, want "NUL"`, e1["1"])
	}
}

func TestParseWEVTTemplate_utf16leWithBom(t *testing.T) {
	// UTF-16LE with BOM (0xFF 0xFE) — should continue to work
	xml := `<template>
  <event value="200">
    <data name="ParamA"/>
  </event>
</template>`
	bom := []byte{0xFF, 0xFE}
	utf16Data := append(bom, utf16le(xml)...)

	result := ParseWEVTTemplate(utf16Data)
	if result == nil {
		t.Fatal("ParseWEVTTemplate(UTF16LE with BOM) = nil, want parsed")
	}
	e200, ok := result["200"]
	if !ok {
		t.Fatal("expected event 200")
	}
	if e200["1"] != "ParamA" {
		t.Fatalf(`e200["1"] = %q, want "ParamA"`, e200["1"])
	}
}

func TestParseWEVTTemplate_utf8(t *testing.T) {
	// Plain UTF-8 XML — should continue to work (multi-line, the parser splits by \n)
	xml := []byte("<template>\n  <event value=\"300\">\n    <data name=\"X\"/>\n  </event>\n</template>")
	result := ParseWEVTTemplate(xml)
	if result == nil {
		t.Fatal("ParseWEVTTemplate(UTF-8) = nil, want parsed")
	}
	e300, ok := result["300"]
	if !ok {
		t.Fatal("expected event 300")
	}
	if e300["1"] != "X" {
		t.Fatalf(`e300["1"] = %q, want "X"`, e300["1"])
	}
}
