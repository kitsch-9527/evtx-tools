
# EVTX 解析工具修复技术参考手册

## 文档概述
本文档为工程师提供详细的修复技术参考，包括代码示例、调试技巧和最佳实践。

## 1. 模板匹配修复技术参考

### 1.1 Provider 名称标准化函数

#### **代码实现**
```rust
// resolver.rs 中的标准化函数
fn normalize_provider_name(name: &str) -> String {
    // 移除 "Microsoft-Windows-" 前缀
    let normalized = name.strip_prefix("Microsoft-Windows-").unwrap_or(name);
    normalized.to_string()
}

// 使用示例
let provider1 = "Microsoft-Windows-Kernel-General";
let provider2 = "EventLog";
let normalized1 = normalize_provider_name(provider1); // "Kernel-General"
let normalized2 = normalize_provider_name(provider2); // "EventLog"
```

#### **测试用例**
```rust
#[test]
fn test_normalize_provider_name() {
    assert_eq!(normalize_provider_name("Microsoft-Windows-Kernel-General"), "Kernel-General");
    assert_eq!(normalize_provider_name("EventLog"), "EventLog");
    assert_eq!(normalize_provider_name("Microsoft-Windows-Security-Auditing"), "Security-Auditing");
}
```

### 1.2 语言回退机制

#### **代码实现**
```rust
// 修改 warmup_lang() 添加语言回退
pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
    // 1. 先尝试精确匹配
    // 2. 如果失败，尝试标准化后的匹配
    // 3. 最后尝试语言回退 (zh-CN -> en-US)
    
    for (provider, event_id) in pairs {
        // 尝试精确匹配
        let key = (provider.clone(), *event_id);
        if self.cache.contains_key(&key) {
            continue;
        }
        
        // 尝试标准化匹配
        let normalized_provider = normalize_provider_name(provider);
        let normalized_key = (normalized_provider, *event_id);
        if self.cache.contains_key(&normalized_key) {
            continue;
        }
        
        // 尝试语言回退
        if lang != "en-US" {
            let en_us_key = (provider.clone(), *event_id);
            // 查询 en-US 版本
            // 如果找到，使用 en-US 版本
        }
    }
    Ok(())
}
```

### 1.3 模糊匹配支持

#### **代码实现**
```rust
// 在 expand_message() 中添加模糊匹配
pub fn expand_message(&self, message: &str, provider: &str) -> String {
    // 1. 尝试精确匹配
    if let Some(refs) = self.param_refs.get(provider) {
        if !refs.is_empty() {
            return expand_param_refs(message, refs);
        }
    }
    
    // 2. 尝试模糊匹配（部分名称匹配）
    for (prov, refs) in &self.param_refs {
        if !refs.is_empty() && (provider.contains(prov) || prov.contains(provider)) {
            return expand_param_refs(message, refs);
        }
    }
    
    // 3. 回退到 'unknown'
    if let Some(refs) = self.param_refs.get("unknown") {
        if !refs.is_empty() {
            return expand_param_refs(message, refs);
        }
    }
    
    message.to_string()
}
```

---

## 2. 值映射修复技术参考

### 2.1 硬编码值映射函数

#### **Kernel-Power 值映射**
```rust
// resolver.rs 中的硬编码映射
fn load_kernel_power_value_maps(&mut self) -> Result<()> {
    let reason_map = HashMap::from([
        ("12".to_string(), "SessionUnlock".to_string()),
        ("32".to_string(), "InputHid".to_string()),
        ("31".to_string(), "InputAccelerometer".to_string()),
        ("33".to_string(), "InputTouchpad".to_string()),
        ("34".to_string(), "InputMouse".to_string()),
    ]);
    self.value_maps.insert("Reason".to_string(), reason_map);
    Ok(())
}
```

#### **Setup 事件值映射**
```rust
fn load_setup_value_maps(&mut self) -> Result<()> {
    let setup_states = HashMap::from([
        ("5000".to_string(), "Absent".to_string()),
        ("5064".to_string(), "Staged".to_string()),
        ("5112".to_string(), "Installed".to_string()),
        ("5120".to_string(), "Installed".to_string()),
        ("5130".to_string(), "Installed".to_string()),
    ]);
    self.value_maps.insert("PackageState".to_string(), setup_states);
    Ok(())
}
```

### 2.2 位图映射处理

#### **代码实现**
```rust
// substitute.rs 中的位图映射处理
fn match_bitmap(value: &str, entries: &HashMap<String, String>) -> Option<String> {
    // 解析数值为位掩码
    let num_value = match value.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return None,
    };
    
    // 查找每个设置位对应的显示名称
    let mut result = Vec::new();
    for (bit_str, display_name) in entries {
        if let Ok(bit) = bit_str.parse::<u64>() {
            if num_value & (1 << bit) != 0 {
                result.push(display_name.clone());
            }
        }
    }
    
    // 返回用逗号连接的显示名称
    if result.is_empty() {
        None
    } else {
        Some(result.join(", "))
    }
}
```

### 2.3 值映射应用函数

#### **代码实现**
```rust
pub fn apply_value_maps(
    params: &[String],
    param_map: &Option<HashMap<String, String>>,
    value_maps: &HashMap<String, HashMap<String, String>>,
    bitmap_names: &HashSet<String>,
) -> Vec<String> {
    let Some(map) = param_map else {
        return params.to_vec();
    };
    if map.is_empty() || value_maps.is_empty() {
        return params.to_vec();
    }

    params
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let idx_str = (i + 1).to_string();
            match map.get(&idx_str) {
                Some(field_name) => match value_maps.get(field_name) {
                    Some(entries) => {
                        if bitmap_names.contains(field_name) {
                            // Bitmap: decompose value into set bits
                            match_bitmap(value, entries)
                                .unwrap_or_else(|| value.clone())
                        } else {
                            // Regular VMAP: exact match
                            entries.get(value)
                                .cloned()
                                .unwrap_or_else(|| value.clone())
                        }
                    }
                    None => value.clone(),
                },
                None => value.clone(),
            }
        })
        .collect()
}
```

---

## 3. 参数替换修复技术参考

### 3.1 GUID 格式化函数

#### **代码实现**
```rust
// substitute.rs 中的 GUID 格式化
fn format_guid(guid: &str) -> String {
    // 根据 Windows 事件查看器的格式进行截断
    if guid.len() > 36 {
        format!("{{{}}}", &guid[1..36])
    } else {
        guid.to_string()
    }
}

// 使用示例
let guid1 = "{55c92734-d682-4d71-983e-d6ec3f16059f}";
let formatted1 = format_guid(guid1); // "{55c92734-d682-4d71-983e-d6ec3f16059f}"

let guid2 = "{55c92734-d682-4d71-983e-d6ec3f16059f-extra}";
let formatted2 = format_guid(guid2); // "{55c92734-d682-4d71-983e-d6ec3f16059f}"
```

### 3.2 两遍扫描逻辑

#### **代码实现**
```rust
pub fn substitute_format_message(template: &str, params: &[&str]) -> String {
    // 第一遍扫描
    let pass1 = substitute(template, params);
    
    // 格式化显示（GUID、十六进制等）
    let normalized = format_for_display(&pass1);
    
    // 第二遍扫描
    substitute(&normalized, params)
}
```

### 3.3 参数引用展开

#### **代码实现**
```rust
pub fn expand_param_refs(message: &str, refs: &HashMap<u32, String>) -> String {
    let mut result = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }

        match chars.next() {
            Some('%') => {
                // Got '%%', now look for digits
                let mut num_str = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if num_str.is_empty() {
                    // '%%' not followed by digits — emit both percents
                    result.push('%');
                    result.push('%');
                } else {
                    let param_id: u32 = match num_str.parse() {
                        Ok(id) => id,
                        Err(_) => {
                            result.push('%');
                            result.push('%');
                            result.push_str(&num_str);
                            continue;
                        }
                    };

                    match refs.get(&param_id) {
                        Some(replacement) => result.push_str(replacement),
                        None => {
                            // Not found — keep as-is for later processing
                            result.push('%');
                            result.push('%');
                            result.push_str(&num_str);
                        }
                    }
                }
            }
            Some(other) => {
                result.push('%');
                result.push(other);
            }
            None => {
                result.push('%');
            }
        }
    }

    result
}
```

---

## 4. 调试技巧

### 4.1 模板匹配调试

#### **调试函数**
```rust
pub fn debug_template_matching(&self, provider: &str, event_id: u32) {
    println!("=== Template Matching Debug ===");
    println!("Provider: {}", provider);
    println!("EventID: {}", event_id);
    
    // 尝试精确匹配
    let key = (provider.to_string(), event_id);
    if let Some(row) = self.cache.get(&key) {
        println!("✅ Exact match found:");
        println!("  Message: {}", row.message);
    } else {
        println!("❌ No exact match found");
    }
    
    // 尝试标准化匹配
    let normalized = provider.strip_prefix("Microsoft-Windows-").unwrap_or(provider);
    let normalized_key = (normalized.to_string(), event_id);
    if let Some(row) = self.cache.get(&normalized_key) {
        println!("✅ Normalized match found:");
        println!("  Message: {}", row.message);
    } else {
        println!("❌ No normalized match found");
    }
}
```

### 4.2 值映射调试

#### **调试函数**
```rust
pub fn debug_value_maps(&self) {
    println!("=== Value Maps Debug ===");
    println!("Total maps: {}", self.value_maps.len());
    for (map_name, entries) in &self.value_maps {
        println!("Map: {} -> {} entries", map_name, entries.len());
        for (value, display) in entries.iter().take(3) {
            println!("  {} -> {}", value, display);
        }
    }
}
```

### 4.3 参数替换调试

#### **调试函数**
```rust
pub fn debug_parameter_extraction(event_data: &JsonValue) {
    println!("=== Parameter Extraction Debug ===");
    if let Some(params) = event_data.get("EventData") {
        if let Some(data) = params.get("Data") {
            match data {
                JsonValue::Array(arr) => {
                    println!("Array format: {} parameters", arr.len());
                    for (i, param) in arr.iter().enumerate() {
                        println!("  %{}: {:?}", i + 1, param);
                    }
                }
                JsonValue::Object(obj) => {
                    println!("Object format:");
                    for (key, value) in obj {
                        println!("  {}: {:?}", key, value);
                    }
                }
                _ => println!("Unknown format"),
            }
        }
    }
}
```

---

## 5. 测试用例

### 5.1 模板匹配测试

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

### 5.2 值映射测试

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

### 5.3 参数替换测试

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

---

## 6. 最佳实践

### 6.1 代码修改最佳实践
1. **渐进式修改**: 一次只修改一个功能模块
2. **测试驱动**: 编写测试用例后再修改代码
3. **文档同步**: 及时更新文档记录修改内容
4. **代码审查**: 修改后进行自我审查

### 6.2 调试最佳实践
1. **使用调试输出**: println! 或 eprintln! 输出调试信息
2. **分步调试**: 逐步验证每个函数的功能
3. **边界测试**: 测试边界条件和异常情况
4. **性能监控**: 监控修改对性能的影响

### 6.3 测试最佳实践
1. **单元测试**: 为每个函数编写单元测试
2. **集成测试**: 测试模块间的交互
3. **回归测试**: 确保修改不破坏现有功能
4. **性能测试**: 验证修改不影响性能

---

**文档版本**: v1.0
**创建时间**: 2026-08-27T03:19:55.441Z
**作者**: researcher
**团队**: evtx-bugfix-team
**用途**: 工程师技术参考手册
