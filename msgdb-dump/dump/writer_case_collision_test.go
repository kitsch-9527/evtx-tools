package dump

import (
	"path/filepath"
	"strings"
	"testing"
)

// R48（2026-10-08）C 契约：msgdb 内不得存在仅大小写不同的双 provider。
// 渲染端（evtx-tools-core R48）装载时对 provider 键做 lowercase 规范化——
// 真撞名会被折叠成同一 cohort，渲染语义静默串线，必须崩在 dump 侧（生产者
// 门）。生产事故镜像：事件 'Disk' vs 注册表/msgdb 'disk' 失配本属无撞名的
// 大小写差异，渲染端修复依赖"库内无真撞名"这一前提。
func TestValidateProviderCaseCollisions(t *testing.T) {
	t.Run("clean db passes", func(t *testing.T) {
		dbPath := filepath.Join(t.TempDir(), "clean.db")
		groups := []ProviderVariantGroup{
			makeTestVariantGroup(`C:\a.dll`, makeTestProvider("disk", "", "System"),
				[]VariantRow{makeTestVariantRow(51, "paging error on %1.")}),
		}
		if err := CreateSystemDB(dbPath, groups, testOsInfo(), []string{"en-US"}); err != nil {
			t.Fatalf("create clean db: %v", err)
		}
		if err := ValidateProviderCaseCollisions(dbPath); err != nil {
			t.Fatalf("clean db must pass validation: %v", err)
		}
	})

	t.Run("case-colliding providers fail", func(t *testing.T) {
		dbPath := filepath.Join(t.TempDir(), "collide.db")
		groups := []ProviderVariantGroup{
			makeTestVariantGroup(`C:\a.dll`, makeTestProvider("disk", "", "System"),
				[]VariantRow{makeTestVariantRow(51, "paging error on %1.")}),
			makeTestVariantGroup(`C:\b.dll`, makeTestProvider("DISK", "", "System"),
				[]VariantRow{makeTestVariantRow(52, "colliding cohort")}),
		}
		if err := CreateSystemDB(dbPath, groups, testOsInfo(), []string{"en-US"}); err != nil {
			t.Fatalf("create colliding db: %v", err)
		}
		err := ValidateProviderCaseCollisions(dbPath)
		if err == nil {
			t.Fatal("providers differing only by case must fail validation")
		}
		if !strings.Contains(strings.ToLower(err.Error()), "case collision") {
			t.Fatalf("error must name the collision, got: %v", err)
		}
	})
}
