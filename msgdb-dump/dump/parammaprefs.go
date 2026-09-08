package dump

import (
	"fmt"

	"github.com/kitsch-9527/msgdb-dump/crim"
)

// R9（2026-09-04）：从 CRIM ProviderData 还原事件参数→值映射链接。
//
// CRIM 源头（TTBL TEMP item +8 MapRef → VMAP/BMAP）在主分支（RT_MESSAGETABLE 有消息）
// 与 fallback 分支都存在，但 enrichFromCRIM 只补 Level/Task/Opcode/Keywords 名，
// 不收集 ParamMapRefs（只在 CRIM+MUI fallback 收集）。实证：Kernel-Power 566 param2
// Reason 的 MapRef=3728→Pop:PowerStateEvent（探针直读系统 DLL），DB 却无 (566,2) 链接，
// 导致 wevtapi 渲染 SessionUnlock 而我们输出裸 12（190 条 D）。
//
// 本函数是 provider_extractor.go:95-112 同逻辑的纯函数 seam，供 enrichFromCRIM
// （主分支）与单测调用；fallback 分支保持原逻辑不变。
func collectParamMapRefs(pd *crim.ProviderData, providerName string, events []crim.EventEntry) []ParamMapRef {
	if pd == nil || pd.TemplateParams == nil {
		return nil
	}
	var refs []ParamMapRef
	for _, ev := range events {
		if ev.TemplateOffset == nil {
			continue
		}
		params, ok := pd.TemplateParams[*ev.TemplateOffset]
		if !ok || len(params) == 0 {
			continue
		}
		for _, p := range params {
			if p.MapRef == 0 {
				continue
			}
			info, ok := pd.MapRefsByOffset[p.MapRef]
			if !ok || info.Name == "" {
				continue
			}
			key := providerName + "." + info.Name
			if info.IsBitmap {
				key += "_bits"
			}
			refs = append(refs, ParamMapRef{
				ProviderName: providerName,
				EventID:      uint32(ev.EventID),
				ParamIndex:   fmt.Sprintf("%d", p.Index),
				ParamName:    p.Name,
				MapName:      key,
				IsBitmap:     info.IsBitmap,
			})
		}
	}
	return refs
}

// collectParamTypes gathers per-parameter TDH InType/OutType bytes (R10) from
// the same CRIM ProviderData. Pairs where both bytes mean "plain string"
// (0x01/0x01) or "unset" (0/0) are skipped — the renderer treats those as
// passthrough anyway, so storing them would only bloat the DB.
func collectParamTypes(pd *crim.ProviderData, providerName string, events []crim.EventEntry) []ParamTypeRef {
	if pd == nil || pd.TemplateParams == nil {
		return nil
	}
	var refs []ParamTypeRef
	for _, ev := range events {
		if ev.TemplateOffset == nil {
			continue
		}
		params, ok := pd.TemplateParams[*ev.TemplateOffset]
		if !ok || len(params) == 0 {
			continue
		}
		for _, p := range params {
			if (p.InType == 0x01 && p.OutType == 0x01) || (p.InType == 0 && p.OutType == 0) {
				continue
			}
			refs = append(refs, ParamTypeRef{
				ProviderName: providerName,
				EventID:      uint32(ev.EventID),
				ParamIndex:   fmt.Sprintf("%d", p.Index),
				ParamName:    p.Name,
				InType:       p.InType,
				OutType:      p.OutType,
			})
		}
	}
	return refs
}
