//! Edge case tests for Code management.

use crate::code::*;

// ═══════════════════════════════════════════════════════════════════════════
// CodePtr: null, is_null, from_raw, as_usize
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_code_ptr_null() {
    let ptr = CodePtr::null();
    assert!(ptr.is_null());
    assert_eq!(ptr.as_usize(), 0);
}

#[test]
fn test_code_ptr_from_raw() {
    let ptr = CodePtr::from_raw(0xDEAD_BEEF);
    assert!(!ptr.is_null());
    assert_eq!(ptr.as_usize(), 0xDEAD_BEEF);
}

#[test]
fn test_code_ptr_from_raw_zero() {
    let ptr = CodePtr::from_raw(0);
    assert!(ptr.is_null());
}

#[test]
fn test_code_ptr_from_raw_max() {
    let ptr = CodePtr::from_raw(usize::MAX);
    assert!(!ptr.is_null());
    assert_eq!(ptr.as_usize(), usize::MAX);
}

#[test]
fn test_code_ptr_equality() {
    let p1 = CodePtr::from_raw(42);
    let p2 = CodePtr::from_raw(42);
    let p3 = CodePtr::from_raw(43);
    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

#[test]
fn test_code_ptr_copy() {
    let p1 = CodePtr::from_raw(42);
    let p2 = p1;
    assert_eq!(p1, p2);
}

#[test]
fn test_code_ptr_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CodePtr::from_raw(1));
    set.insert(CodePtr::from_raw(2));
    set.insert(CodePtr::from_raw(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_code_ptr_debug() {
    let ptr = CodePtr::from_raw(42);
    let dbg = format!("{:?}", ptr);
    assert!(dbg.contains("CodePtr"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// FunctionEntry creation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_function_entry_creation() {
    let entry = FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 42,
    };

    assert!(!entry.code.is_null());
    assert!(entry.is_aot);
    assert_eq!(entry.module, 1);
    assert_eq!(entry.function, 2);
    assert_eq!(entry.arity, 3);
    assert_eq!(entry.file, 4);
    assert_eq!(entry.line, 42);
}

#[test]
fn test_function_entry_clone() {
    let entry = FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: false,
        module: 10,
        function: 20,
        arity: 5,
        file: 30,
        line: 100,
    };

    let entry2 = entry.clone();
    assert_eq!(entry.code, entry2.code);
    assert_eq!(entry.is_aot, entry2.is_aot);
    assert_eq!(entry.module, entry2.module);
    assert_eq!(entry.function, entry2.function);
    assert_eq!(entry.arity, entry2.arity);
    assert_eq!(entry.file, entry2.file);
    assert_eq!(entry.line, entry2.line);
}

#[test]
fn test_function_entry_debug() {
    let entry = FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 5,
    };

    let dbg = format!("{:?}", entry);
    assert!(dbg.contains("FunctionEntry"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// ModuleCode: add_function, lookup, add_export, duplicate function
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_module_code_new() {
    let code = ModuleCode::new(42);
    assert_eq!(code.module, 42);
    assert!(code.functions.is_empty());
    assert!(code.exports.is_empty());
}

#[test]
fn test_module_code_add_function() {
    let mut code = ModuleCode::new(1);

    code.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    });

    assert_eq!(code.functions.len(), 1);
}

#[test]
fn test_module_code_lookup() {
    let mut code = ModuleCode::new(1);

    code.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    });

    let result = code.lookup(10, 2);
    assert!(result.is_some());
    assert_eq!(result.unwrap().code.as_usize(), 0x1000);
}

#[test]
fn test_module_code_lookup_nonexistent() {
    let code = ModuleCode::new(1);
    assert!(code.lookup(99, 1).is_none());
}

#[test]
fn test_module_code_lookup_wrong_arity() {
    let mut code = ModuleCode::new(1);

    code.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    });

    // Same function name, different arity
    assert!(code.lookup(10, 3).is_none());
}

#[test]
fn test_module_code_add_export() {
    let mut code = ModuleCode::new(1);

    code.add_export(10, 2);
    assert_eq!(code.exports.len(), 1);
    assert_eq!(code.exports[0], (10, 2));
}

#[test]
fn test_module_code_add_multiple_exports() {
    let mut code = ModuleCode::new(1);

    code.add_export(10, 2);
    code.add_export(20, 3);
    code.add_export(30, 1);

    assert_eq!(code.exports.len(), 3);
}

#[test]
fn test_module_code_duplicate_function() {
    let mut code = ModuleCode::new(1);

    let entry1 = FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    };

    let entry2 = FunctionEntry {
        code: CodePtr::from_raw(0x2000),
        is_aot: false,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 10,
    };

    code.add_function(entry1);
    code.add_function(entry2); // Should overwrite

    assert_eq!(code.functions.len(), 1);
    let result = code.lookup(10, 2);
    assert!(result.is_some());
    // The second entry should have overwritten the first
    assert_eq!(result.unwrap().code.as_usize(), 0x2000);
}

#[test]
fn test_module_code_multiple_functions() {
    let mut code = ModuleCode::new(1);

    for i in 0..10 {
        code.add_function(FunctionEntry {
            code: CodePtr::from_raw(0x1000 + i * 0x100),
            is_aot: true,
            module: 1,
            function: i as u64,
            arity: 2,
            file: 0,
            line: i as u32,
        });
    }

    assert_eq!(code.functions.len(), 10);

    for i in 0..10 {
        let result = code.lookup(i as u64, 2);
        assert!(result.is_some());
        assert_eq!(result.unwrap().code.as_usize(), 0x1000 + i * 0x100);
    }
}

#[test]
fn test_module_code_default() {
    let code: ModuleCode = Default::default();
    assert!(code.functions.is_empty());
    assert!(code.exports.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// CodeRegistry: register_module, lookup, modules list
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_code_registry_new() {
    let registry = CodeRegistry::new();
    assert!(registry.modules().is_empty());
}

#[test]
fn test_code_registry_register_module() {
    let registry = CodeRegistry::new();
    let module = ModuleCode::new(42);

    registry.register_module(42, module);
    let modules = registry.modules();
    assert_eq!(modules.len(), 1);
    assert!(modules.contains(&42));
}

#[test]
fn test_code_registry_register_multiple_modules() {
    let registry = CodeRegistry::new();

    for i in 0..5 {
        registry.register_module(i, ModuleCode::new(i));
    }

    let modules = registry.modules();
    assert_eq!(modules.len(), 5);
}

#[test]
fn test_code_registry_lookup() {
    let registry = CodeRegistry::new();
    let mut module = ModuleCode::new(1);

    module.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    });

    registry.register_module(1, module);

    let result = registry.lookup(1, 10, 2);
    assert!(result.is_some());
    assert_eq!(result.unwrap().code.as_usize(), 0x1000);
}

#[test]
fn test_code_registry_lookup_nonexistent_module() {
    let registry = CodeRegistry::new();
    assert!(registry.lookup(99, 10, 2).is_none());
}

#[test]
fn test_code_registry_lookup_nonexistent_function() {
    let registry = CodeRegistry::new();
    let module = ModuleCode::new(1);
    registry.register_module(1, module);

    assert!(registry.lookup(1, 99, 2).is_none());
}

#[test]
fn test_code_registry_modules_list() {
    let registry = CodeRegistry::new();

    registry.register_module(10, ModuleCode::new(10));
    registry.register_module(20, ModuleCode::new(20));

    let modules = registry.modules();
    assert_eq!(modules.len(), 2);
    assert!(modules.contains(&10));
    assert!(modules.contains(&20));
}

#[test]
fn test_code_registry_overwrite_module() {
    let registry = CodeRegistry::new();

    let mut module1 = ModuleCode::new(1);
    module1.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x1000),
        is_aot: true,
        module: 1,
        function: 10,
        arity: 2,
        file: 0,
        line: 1,
    });

    let mut module2 = ModuleCode::new(1);
    module2.add_function(FunctionEntry {
        code: CodePtr::from_raw(0x2000),
        is_aot: true,
        module: 1,
        function: 20,
        arity: 3,
        file: 0,
        line: 2,
    });

    registry.register_module(1, module1);
    registry.register_module(1, module2); // Overwrite

    // Old function should not be found
    assert!(registry.lookup(1, 10, 2).is_none());
    // New function should be found
    assert!(registry.lookup(1, 20, 3).is_some());
}

#[test]
fn test_code_registry_default() {
    let registry: CodeRegistry = Default::default();
    assert!(registry.modules().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// LazyFnRef: new, get/set, is_resolved
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_lazy_fn_ref_new() {
    let lazy = LazyFnRef::new(1, 2, 3);
    assert!(!lazy.is_resolved());
    assert!(lazy.get().is_null());
}

#[test]
fn test_lazy_fn_ref_set() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::from_raw(0xDEAD_BEEF));

    assert!(lazy.is_resolved());
    assert_eq!(lazy.get().as_usize(), 0xDEAD_BEEF);
}

#[test]
fn test_lazy_fn_ref_set_null() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::null());

    assert!(!lazy.is_resolved());
}

#[test]
fn test_lazy_fn_ref_overwrite() {
    let lazy = LazyFnRef::new(1, 2, 3);

    lazy.set(CodePtr::from_raw(0x1000));
    assert_eq!(lazy.get().as_usize(), 0x1000);

    lazy.set(CodePtr::from_raw(0x2000));
    assert_eq!(lazy.get().as_usize(), 0x2000);
}

#[test]
fn test_lazy_fn_ref_get_before_set() {
    let lazy = LazyFnRef::new(1, 2, 3);
    // get() before set() should return null
    let ptr = lazy.get();
    assert!(ptr.is_null());
}
