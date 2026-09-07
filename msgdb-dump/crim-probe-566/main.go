package main

import (
	"encoding/binary"
	"fmt"
	"os"
	"strings"

	"github.com/kitsch-9527/msgdb-dump/pe"
)

func main() {
	dll := `C:\WINDOWS\system32\drivers\HTTP.SYS`
	if len(os.Args) > 1 {
		dll = os.Args[1]
	}
	combined, err := pe.ExtractAllFromPE(dll)
	if err != nil {
		fmt.Println("extract err:", err)
		os.Exit(1)
	}
	for _, b := range combined.CrimBlobs {
		if len(b) < 4 || string(b[:4]) != "CRIM" {
			continue
		}
		for pos := 0; pos+12 < len(b); pos += 4 {
			if string(b[pos:pos+4]) != "TTBL" {
				continue
			}
		tempCount := binary.LittleEndian.Uint32(b[pos+8 : pos+12])
		cur := pos + 12
		for i := 0; i < int(tempCount); i++ {
			if cur+40 > len(b) || string(b[cur:cur+4]) != "TEMP" {
			break
			}
			tempSize := int(binary.LittleEndian.Uint32(b[cur+4 : cur+8]))
			itemDescCount := int(binary.LittleEndian.Uint32(b[cur+8 : cur+12]))
			itemsOffset := int(binary.LittleEndian.Uint32(b[cur+16 : cur+20]))
			if itemDescCount == 5 && itemsOffset+itemDescCount*20 <= len(b) {
				names := []string{}
			for idx := 0; idx < itemDescCount; idx++ {
				dOff := itemsOffset + idx*20
				flags := binary.LittleEndian.Uint32(b[dOff : dOff+4])
				typeRaw := binary.LittleEndian.Uint32(b[dOff+4 : dOff+8])
				mapRef := binary.LittleEndian.Uint32(b[dOff+8 : dOff+12])
				u12 := binary.LittleEndian.Uint32(b[dOff+12 : dOff+16])
				nameOff := int(binary.LittleEndian.Uint32(b[dOff+16 : dOff+20]))
				nm := ""
				if nameOff > 0 && nameOff < len(b) {
					end := nameOff
					for end < len(b) && b[end] != 0 {
						end++
					}
					nm = string(b[nameOff:end])
				}
				names = append(names, nm)
				fmt.Printf("TEMP@%d param%d name=%s flags=%08x type=%08x mapRef=%d u12=%d\n", cur, idx+1, nm, flags, typeRaw, mapRef, u12)
			}
			if strings.Join(names, ",") == "Url,ReserveStatus,ProcessId,ExecutablePath,UserSid" {
				fmt.Println("FOUND EID112 TEMP")
				os.Exit(0)
			}
			if tempSize > 0 {
				cur += tempSize
			} else {
				cur += 40
			}
		}
	}
	fmt.Println("not found")
}
}
