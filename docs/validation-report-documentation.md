
# EVTX 解析工具验证报告文档

## 文档概述
本文档记录了 EVTX 解析工具修复的验证方法、测试用例、验证结果和后续改进建议。

## 1. 验证概述

### 1.1 验证目标
1. **功能验证**: 确保修复后的功能正常工作
2. **兼容性验证**: 确保修复不影响现有功能
3. **性能验证**: 确保修复不会显著影响性能
4. **回归验证**: 确保修复不会引入新问题

### 1.2 验证范围
1. **模板匹配验证**: EventID 24, 6013
2. **值映射验证**: EventID 566, Setup 通道事件
3. **参数替换验证**: EventID 8233, 8230
4. **整体功能验证**: 所有 EventID 的解析

---

## 2. 验证方法

### 2.1 单元测试

#### **模板匹配测试**
```rust
#[test]
fn test_eventid_24_template() {
    let expected = "The time zone information was refreshed with exit reason 0. Current time zone bias is -480.";
    let actual = get_message_for_event("Microsoft-Windows-Kernel-General", 24);
    assert_eq!(actual, expected, "EventID 24 template mismatch");
}

#[test]
fn test_eventid_6013_template() {
    let expected = "The system uptime is 354558 seconds.";
    let actual = get_message_for_event("EventLog", 6013);
    assert_eq!(actual, expected, "EventID 6013 template mismatch");
}
```

#### **值映射测试**
```rust
#[test]
fn test_eventid_566_value_mapping() {
    let params = vec!["12".to_string()];
    let param_map = Some(HashMap::from([("1".to_string(), "Reason".to_string())]));
    let value_maps = HashMap::from([
        ("Reason".to_string(), HashMap::from([
            ("12".to_string(), "SessionUnlock".to_string()),
        ])),
    ]);
    
    let result = apply_value_maps(&params, &param_map, &value_maps, &HashSet::new());
    assert_eq!(result, vec!["SessionUnlock"]);
}

#[test]
fn test_setup_value_mapping() {
    let params = vec!["5112".to_string()];
    let param_map = Some(HashMap::from([("1".to_string(), "PackageState".to_string())]));
    let value_maps = HashMap::from([
        ("PackageState".to_string(), HashMap::from([
            ("5112".to_string(), "Installed".to_string()),
        ])),
    ]);
    
    let result = apply_value_maps(&params, &param_map, &value_maps, &HashSet::new());
    assert_eq!(result, vec!["Installed"]);
}
```

#### **参数替换测试**
```rust
#[test]
fn test_parameter_substitution() {
    let template = "User %1 logged on from %2";
    let params = vec!["admin", "192.168.1.1"];
    
    let result = substitute(template, &params);
    assert_eq!(result, "User admin logged on from 192.168.1.1");
}

#[test]
fn test_guid_formatting() {
    let guid = "{55c92734-d682-4d71-983e-d6ec3f16059f}";
    let formatted = format_for_display(guid);
    
    assert!(formatted.starts_with("{"));
    assert!(formatted.ends_with("}"));
    assert!(formatted.contains("-"));
}
```

### 2.2 集成测试

#### **端到端测试**
```bash
# 1. 编译项目
cargo build --release

# 2. 准备测试数据
cp test-logs/System.evtx test.evtx

# 3. 运行解析
./target/release/evtx-tools parse test.evtx \
  --msg-db messages.db \
  --format sqlite \
  --output test_output.db

# 4. 检查输出
sqlite3 test_output.db "SELECT * FROM events LIMIT 10;"
```

#### **对比测试**
```bash
# 对比 evtx-tools 输出与 Windows 事件查看器输出
# 1. 使用 evtx-tools 解析
./target/release/evtx-tools parse test.evtx --output evtx_tools_output.txt

# 2. 使用 Windows 事件查看器导出
wevtutil qe Security /f:text > windows_output.txt

# 3. 对比输出
diff evtx_tools_output.txt windows_output.txt
```

### 2.3 性能测试

#### **性能基准测试**
```rust
#[bench]
fn bench_parsing_performance(b: &mut Bencher) {
    b.iter(|| {
        // 测试解析性能
        parse_evtx_file("test.evtx");
    });
}
```

#### **内存使用测试**
```rust
#[test]
fn test_memory_usage() {
    // 测试大文件解析时的内存使用
    let memory_before = get_memory_usage();
    parse_evtx_file("large_test.evtx");
    let memory_after = get_memory_usage();
    
    assert!(memory_after - memory_before < 1024 * 1024 * 100); // 100MB 内存限制
}
```

---

## 3. 测试用例

### 3.1 模板匹配测试用例

#### **测试用例 1: EventID 24 (Kernel-General)**
- **输入**: Microsoft-Windows-Kernel-General, EventID 24
- **预期输出**: "The time zone information was refreshed with exit reason 0. Current time zone bias is -480."
- **测试目的**: 验证模板匹配正确性

#### **测试用例 2: EventID 6013 (EventLog)**
- **输入**: EventLog, EventID 6013
- **预期输出**: "The system uptime is 354558 seconds."
- **测试目的**: 验证模板匹配正确性

### 3.2 值映射测试用例

#### **测试用例 3: EventID 566 (Kernel-Power)**
- **输入**: Reason 字段值 "12"
- **预期输出**: "SessionUnlock"
- **测试目的**: 验证值映射正确性

#### **测试用例 4: Setup 通道事件**
- **输入**: PackageState 字段值 "5112"
- **预期输出**: "Installed"
- **测试目的**: 验证 Setup 值映射正确性

### 3.3 参数替换测试用例

#### **测试用例 5: 基本参数替换**
- **输入**: 模板 "User %1 logged on from %2", 参数 ["admin", "192.168.1.1"]
- **预期输出**: "User admin logged on from 192.168.1.1"
- **测试目的**: 验证基本参数替换功能

#### **测试用例 6: GUID 格式化**
- **输入**: GUID "{55c92734-d682-4d71-983e-d6ec3f16059f}"
- **预期输出**: 格式化的 GUID
- **测试目的**: 验证 GUID 格式化功能

---

## 4. 验证结果

### 4.1 当前验证状态

| 测试用例 | 状态 | 验证结果 | 备注 |
|----------|------|----------|------|
| EventID 24 模板匹配 | ❌ 未验证 | 待测试 | 需要修复后验证 |
| EventID 6013 模板匹配 | ❌ 未验证 | 待测试 | 需要修复后验证 |
| EventID 566 值映射 | ❌ 未验证 | 待测试 | 需要修复后验证 |
| Setup 值映射 | ❌ 未验证 | 待测试 | 需要修复后验证 |
| EventID 8230 GUID格式 | ❌ 未验证 | 待测试 | 需要修复后验证 |

### 4.2 历史验证结果

#### **t4 验证报告 (2026-08-25)**
- **EventID 7040**: ✅ 通过
- **EventID 566**: ⚠️ 部分通过 (值映射缺失)
- **EventID 16384**: ✅ 通过
- **Setup 事件**: ✅ 通过 (使用 zh-CN)

#### **t11 验证报告 (2026-08-26)**
- **EventID 24**: ❌ 失败 (模板错误)
- **EventID 6013**: ❌ 失败 (模板错误)
- **EventID 566**: ❌ 失败 (值映射缺失)
- **Setup 事件**: ❌ 失败 (值映射回归)

### 4.3 验证数据分析

#### **问题统计**
1. **模板匹配问题**: 2 个 EventID (24, 6013)
2. **值映射问题**: 2 个 EventID (566, Setup)
3. **参数格式问题**: 1 个 EventID (8230)

#### **影响范围**
1. **高影响**: EventID 24, 6013, Setup (影响系统事件解析)
2. **中影响**: EventID 566 (影响事件可读性)
3. **低影响**: EventID 8230 (轻微格式差异)

---

## 5. 后续改进建议

### 5.1 短期改进 (1-2 周)

#### **优先级 1: 完善测试用例**
1. **添加边界测试**
   - 测试缺失参数的情况
   - 测试空值映射的情况
   - 测试语言不可用的情况

2. **添加性能测试**
   - 测试大文件解析性能
   - 测试内存使用情况
   - 测试并发解析性能

#### **优先级 2: 建立自动化测试**
1. **CI/CD 集成**
   - 将测试集成到 CI/CD 流程
   - 自动运行测试用例
   - 生成测试报告

2. **回归测试**
   - 建立回归测试套件
   - 每次修复后运行回归测试
   - 确保修复不引入新问题

### 5.2 中期改进 (1-2 个月)

#### **测试框架完善**
1. **测试数据管理**
   - 建立测试数据集
   - 管理测试用例版本
   - 支持多语言测试数据

2. **测试工具开发**
   - 开发测试辅助工具
   - 支持测试结果可视化
   - 提供测试覆盖率报告

#### **验证流程优化**
1. **验证标准制定**
   - 制定详细的验证标准
   - 建立质量门禁
   - 支持验证结果追溯

2. **验证环境管理**
   - 建立标准化的验证环境
   - 支持多环境验证
   - 管理验证环境配置

### 5.3 长期改进 (3-6 个月)

#### **测试文化建设**
1. **测试培训**
   - 培训团队测试技能
   - 推广测试驱动开发
   - 建立测试质量意识

2. **测试社区**
   - 建立测试社区
   - 分享测试经验
   - 推广最佳实践

#### **测试生态系统**
1. **测试工具链**
   - 开发完整的测试工具链
   - 支持多种测试类型
   - 提供测试分析报告

2. **测试数据生态**
   - 建立测试数据共享平台
   - 支持社区贡献测试数据
   - 管理测试数据质量

---

## 6. 验证工具和资源

### 6.1 测试工具
1. **cargo test**: Rust 单元测试框架
2. **SQLite**: 数据库验证工具
3. **diff**: 文件对比工具
4. **perf**: 性能分析工具

### 6.2 测试资源
1. **测试数据**: test-logs/ 目录下的 EVTX 文件
2. **参考数据**: Windows 事件查看器输出
3. **文档资源**: docs/ 目录下的技术文档

### 6.3 测试环境
1. **开发环境**: Windows 10/11, Rust 1.70+
2. **测试环境**: 标准化的测试环境
3. **生产环境**: 与生产环境一致的验证环境

---

## 7. 验证报告模板

### 7.1 报告格式
```
# 验证报告

## 基本信息
- 验证日期: YYYY-MM-DD
- 验证人员: [姓名]
- 验证环境: [环境描述]

## 验证结果
| 测试用例 | 状态 | 结果 | 备注 |
|----------|------|------|------|
| [用例1] | [✅/❌] | [结果] | [备注] |

## 问题记录
1. [问题1]: [描述]
2. [问题2]: [描述]

## 改进建议
1. [建议1]: [描述]
2. [建议2]: [描述]

## 结论
[总结验证结果和后续行动]
```

### 7.2 报告提交
1. **报告保存**: 保存到 test-logs/ 目录
2. **报告分享**: 分享给团队成员
3. **报告存档**: 归档到文档系统

---

**文档版本**: v1.0
**创建时间**: 2026-08-27T03:06:06.292Z
**作者**: researcher
**团队**: evtx-bugfix-team
