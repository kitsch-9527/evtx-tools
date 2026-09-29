package dump

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// 阶段 E（2026-09-14）：NTSTATUS 消息文本表（"NTSTATUS-System-Error" 组）。
//
// Ntfs 140 `Failure status` 参数（InType 0x14 HEXINT32 / OutType 0x1F）在
// wevtutil/TdhFormatProperty 基线中渲染为 ntdll 消息表文本（实测
// 0xC000000E → "A device which does not exist was specified."，zh 环境
// 亦为英文文本）。R15 的 FormatMessage(FROM_SYSTEM) 只盖 Win32 码；
// NTSTATUS 稀疏散布（ntdll 表 2975 条、37 个 64K 簇、280 万扫描空洞），
// 暴力 FormatMessage 不可行——这里直接解析 ntdll.dll 的 RT_MESSAGETABLE
// （resource type 11；与 evtx-parity-test/scripts/probe-6013-msgtable.py
// 同一解析逻辑）。写库沿用 R15 模式：全局 provider 组入 parameter_refs，
// 渲染端查表命中才替换。
//
// 注意：取的是二进制内嵌表（英语）。基线（zh 环境 wevtutil）对该表同样
// 输出英文（ntdll 的 MUI 卫星不含此表/回落英语），口径一致。

// collectNtstatusMessages 解析 %SystemRoot%\System32\ntdll.dll 的
// RT_MESSAGETABLE，返回 id→文本（文本去首尾空白）。
func collectNtstatusMessages() (map[uint32]string, error) {
	path := filepath.Join(os.Getenv("SystemRoot"), "System32", "ntdll.dll")
	return collectNtstatusMessagesFromPath(path)
}

// collectNtstatusMessagesFromPath 同上，但接受任意 PE 路径（v9：供 ntdll 的
// MUI companion 萃取臂使用；该臂自守卫——MUI 无消息表时返回空，调用方跳过）。
func collectNtstatusMessagesFromPath(path string) (map[uint32]string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read ntdll: %w", err)
	}
	blobs, err := messageTableResources(data)
	if err != nil {
		return nil, fmt.Errorf("ntdll resources: %w", err)
	}
	out := make(map[uint32]string)
	for _, blob := range blobs {
		ids, texts, err := parseMessageTable(blob)
		if err != nil {
			continue
		}
		for i, id := range ids {
			t := strings.TrimSpace(texts[i])
			if t != "" {
				out[id] = t
			}
		}
	}
	if len(out) == 0 {
		return nil, fmt.Errorf("no message table entries found in %s", path)
	}
	return out, nil
}

// messageTableResources 定位 PE 资源目录并返回全部 RT_MESSAGETABLE (type 11)
// 数据 blob（多语言条目全收）。
func messageTableResources(data []byte) ([][]byte, error) {
	if len(data) < 0x40 {
		return nil, fmt.Errorf("truncated DOS header")
	}
	e_lfanew := int(binary.LittleEndian.Uint32(data[0x3C:]))
	if e_lfanew+4+20 > len(data) || !bytes.Equal(data[e_lfanew:e_lfanew+4], []byte("PE\x00\x00")) {
		return nil, fmt.Errorf("not a PE image")
	}
	coff := e_lfanew + 4
	numSections := int(binary.LittleEndian.Uint16(data[coff+2:]))
	optSize := int(binary.LittleEndian.Uint16(data[coff+16:]))
	opt := coff + 20
	if opt+2 > len(data) {
		return nil, fmt.Errorf("truncated optional header")
	}
	magic := binary.LittleEndian.Uint16(data[opt:])
	var ddir int
	if magic == 0x20B { // PE32+
		ddir = opt + 112
	} else {
		ddir = opt + 96
	}
	if ddir+16 > len(data) {
		return nil, fmt.Errorf("truncated data directories")
	}
	rvaRsrc := binary.LittleEndian.Uint32(data[ddir+2*8:])
	if rvaRsrc == 0 {
		return nil, fmt.Errorf("no resource directory")
	}
	secTab := opt + optSize
	type sec struct{ vaddr, vsize, raddr, rsize uint32 }
	sections := make([]sec, 0, numSections)
	for i := 0; i < numSections; i++ {
		so := secTab + i*40
		if so+40 > len(data) {
			return nil, fmt.Errorf("truncated section table")
		}
		vsize, vaddr, rsize, raddr := binary.LittleEndian.Uint32(data[so+8:]),
			binary.LittleEndian.Uint32(data[so+12:]),
			binary.LittleEndian.Uint32(data[so+16:]),
			binary.LittleEndian.Uint32(data[so+20:])
		sections = append(sections, sec{vaddr, vsize, raddr, rsize})
	}
	rva2off := func(rva uint32) (int, bool) {
		for _, s := range sections {
			if rva >= s.vaddr && rva < s.vaddr+maxU32(s.vsize, s.rsize) {
				return int(s.raddr + (rva - s.vaddr)), true
			}
		}
		return 0, false
	}
	base, ok := rva2off(rvaRsrc)
	if !ok {
		return nil, fmt.Errorf("resource dir RVA unmapped")
	}

	// 三层目录：level1=type，level2=name，level3=lang→data entry。
	var walk func(off int, level int) error
	var blobs [][]byte
	walk = func(off int, level int) error {
		if off+16 > len(data) {
			return fmt.Errorf("truncated resource dir")
		}
		nnamed, nids := int(binary.LittleEndian.Uint16(data[off+12:])),
			int(binary.LittleEndian.Uint16(data[off+14:]))
		for i := 0; i < nnamed+nids; i++ {
			eo := off + 16 + i*8
			if eo+8 > len(data) {
				return fmt.Errorf("truncated dir entry")
			}
			nm, ofs := binary.LittleEndian.Uint32(data[eo:]),
				binary.LittleEndian.Uint32(data[eo+4:])
			if ofs&0x80000000 != 0 {
				sub := base + int(ofs&0x7FFFFFFF)
				ident := int(nm & 0x7FFFFFFF)
				if level == 1 && ident != 11 { // RT_MESSAGETABLE
					continue
				}
				if level == 3 {
					continue
				}
				if err := walk(sub, level+1); err != nil {
					return err
				}
			} else if level == 3 {
				de := base + int(ofs)
				if de+16 > len(data) {
					return fmt.Errorf("truncated data entry")
				}
				dva, dsz := binary.LittleEndian.Uint32(data[de:]),
					binary.LittleEndian.Uint32(data[de+4:])
				boff, ok := rva2off(dva)
				if !ok || boff+int(dsz) > len(data) {
					continue
				}
				blobs = append(blobs, data[boff:boff+int(dsz)])
			}
		}
		return nil
	}
	if err := walk(base, 1); err != nil {
		return nil, err
	}
	return blobs, nil
}

// parseMessageTable 解析 MESSAGE_TABLE 数据块（RPC 格式：块数 + 每块
// low/high/offset，条目 len/flags/text；flags 1=UTF-16，0=ANSI）。
func parseMessageTable(blob []byte) ([]uint32, []string, error) {
	if len(blob) < 4 {
		return nil, nil, fmt.Errorf("truncated message table")
	}
	nblocks := int(binary.LittleEndian.Uint32(blob))
	var ids []uint32
	var texts []string
	for i := 0; i < nblocks; i++ {
		h := 4 + i*12
		if h+12 > len(blob) {
			return nil, nil, fmt.Errorf("truncated block header")
		}
		low, high, entOff := binary.LittleEndian.Uint32(blob[h:]),
			binary.LittleEndian.Uint32(blob[h+4:]),
			binary.LittleEndian.Uint32(blob[h+8:])
		p := int(entOff)
		for mid := low; mid <= high; mid++ {
			if p+4 > len(blob) {
				break
			}
			length, flags := int(binary.LittleEndian.Uint16(blob[p:])),
				binary.LittleEndian.Uint16(blob[p+2:])
			if length < 4 || p+length > len(blob) {
				break
			}
			txt := blob[p+4 : p+length]
			var s string
			if flags == 1 {
				s = strings.TrimRight(utf16ToString(txt), "\x00")
			} else {
				s = string(bytes.TrimRight(txt, "\x00"))
			}
			ids = append(ids, mid)
			texts = append(texts, s)
			p += length
		}
	}
	return ids, texts, nil
}

func utf16ToString(b []byte) string {
	var sb strings.Builder
	for i := 0; i+1 < len(b); i += 2 {
		r := rune(binary.LittleEndian.Uint16(b[i:]))
		sb.WriteRune(r)
	}
	return sb.String()
}

func maxU32(a, b uint32) uint32 {
	if a > b {
		return a
	}
	return b
}

