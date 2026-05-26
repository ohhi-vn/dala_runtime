//! Edge case tests for dala_dispatch.

use super::*;
use crate::export_table::{ExportEntry as TableExportEntry, ExportKey};
use dala_ir::IRModule;
use dala_runtime::code::CodePtr;

// ============================================================
// ExportKey tests
// ============================================================

#[test]
fn test_export_key_creation() {
    let key = ExportKey {
        module: 1,
        function: 2,
        arity: 3,
    };
    assert_eq!(key.module, 1);
    assert_eq!(key.function, 2);
    assert_eq!(key.arity, 3);
}

#[test]
fn test_export_key_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let key1 = ExportKey {
        module: 10,
        function: 20,
        arity: 2,
    };
    let key2 = ExportKey {
        module: 10,
        function: 20,
        arity: 2,
    };

    let mut hasher1 = DefaultHasher::new();
    key1.hash(&mut hasher1);
    let hash1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    key2.hash(&mut hasher2);
    let hash2 = hasher2.finish();

    assert_eq!(hash1, hash2);
}

#[test]
fn test_export_key_hash_different() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let key1 = ExportKey {
        module: 10,
        function: 20,
        arity: 2,
    };
    let key2 = ExportKey {
        module: 10,
        function: 20,
        arity: 3,
    };

    let mut hasher1 = DefaultHasher::new();
    key1.hash(&mut hasher1);
    let hash1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    key2.hash(&mut hasher2);
    let hash2 = hasher2.finish();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_export_key_eq() {
    let key1 = ExportKey {
        module: 5,
        function: 10,
        arity: 1,
    };
    let key2 = ExportKey {
        module: 5,
        function: 10,
        arity: 1,
    };
    let key3 = ExportKey {
        module: 5,
        function: 10,
        arity: 2,
    };

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

#[test]
fn test_export_key_zero_values() {
    let key = ExportKey {
        module: 0,
        function: 0,
        arity: 0,
    };
    assert_eq!(key.module, 0);
    assert_eq!(key.function, 0);
    assert_eq!(key.arity, 0);
}

// ============================================================
// ExportEntry (table) tests
// ============================================================

#[test]
fn test_table_export_entry_new() {
    let ptr = CodePtr::from_raw(0x1000);
    let entry = TableExportEntry::new(ptr);
    assert_eq!(entry.code_ptr, ptr);
}

#[test]
fn test_table_export_entry_new_null() {
    let ptr = CodePtr::null();
    let entry = TableExportEntry::new(ptr);
    assert!(entry.code_ptr.is_null());
}

// ============================================================
// ExportTable tests
// ============================================================

#[test]
fn test_export_table_new() {
    let table = ExportTable::new();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
}

#[test]
fn test_export_table_default() {
    let table = ExportTable::default();
    assert!(table.is_empty());
}

#[test]
fn test_export_table_register() {
    let table = ExportTable::new();
    let ptr = CodePtr::from_raw(0x1000);
    table.register(1, 2, 3, ptr);
    assert_eq!(table.len(), 1);
    assert!(!table.is_empty());
}

#[test]
fn test_export_table_register_multiple() {
    let table = ExportTable::new();
    table.register(1, 1, 0, CodePtr::from_raw(0x1000));
    table.register(1, 2, 1, CodePtr::from_raw(0x2000));
    table.register(2, 1, 0, CodePtr::from_raw(0x3000));
    assert_eq!(table.len(), 3);
}

#[test]
fn test_export_table_lookup_hit() {
    let table = ExportTable::new();
    let ptr = CodePtr::from_raw(0xABCD);
    table.register(10, 20, 2, ptr);

    let result = table.lookup(10, 20, 2);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), ptr);
}

#[test]
fn test_export_table_lookup_miss_wrong_module() {
    let table = ExportTable::new();
    table.register(10, 20, 2, CodePtr::from_raw(0xABCD));

    assert!(table.lookup(99, 20, 2).is_none());
}

#[test]
fn test_export_table_lookup_miss_wrong_function() {
    let table = ExportTable::new();
    table.register(10, 20, 2, CodePtr::from_raw(0xABCD));

    assert!(table.lookup(10, 99, 2).is_none());
}

#[test]
fn test_export_table_lookup_miss_wrong_arity() {
    let table = ExportTable::new();
    table.register(10, 20, 2, CodePtr::from_raw(0xABCD));

    assert!(table.lookup(10, 20, 99).is_none());
}

#[test]
fn test_export_table_lookup_miss_empty_table() {
    let table = ExportTable::new();
    assert!(table.lookup(1, 1, 1).is_none());
}

#[test]
fn test_export_table_remove_hit() {
    let table = ExportTable::new();
    table.register(10, 20, 2, CodePtr::from_raw(0xABCD));
    assert_eq!(table.len(), 1);

    let removed = table.remove(10, 20, 2);
    assert!(removed);
    assert!(table.is_empty());
}

#[test]
fn test_export_table_remove_miss() {
    let table = ExportTable::new();
    table.register(10, 20, 2, CodePtr::from_raw(0xABCD));

    let removed = table.remove(99, 99, 99);
    assert!(!removed);
    assert_eq!(table.len(), 1);
}

#[test]
fn test_export_table_remove_miss_empty() {
    let table = ExportTable::new();
    let removed = table.remove(1, 1, 1);
    assert!(!removed);
}

#[test]
fn test_export_table_len() {
    let table = ExportTable::new();
    assert_eq!(table.len(), 0);

    table.register(1, 1, 0, CodePtr::null());
    assert_eq!(table.len(), 1);

    table.register(1, 2, 1, CodePtr::null());
    assert_eq!(table.len(), 2);

    // Re-registering the same key should overwrite
    table.register(1, 1, 0, CodePtr::from_raw(0x100));
    assert_eq!(table.len(), 2);
}

#[test]
fn test_export_table_is_empty() {
    let table = ExportTable::new();
    assert!(table.is_empty());

    table.register(1, 1, 0, CodePtr::null());
    assert!(!table.is_empty());

    table.remove(1, 1, 0);
    assert!(table.is_empty());
}

#[test]
fn test_export_table_module_exports() {
    let table = ExportTable::new();
    table.register(1, 10, 0, CodePtr::from_raw(0x1000));
    table.register(1, 20, 1, CodePtr::from_raw(0x2000));
    table.register(2, 30, 2, CodePtr::from_raw(0x3000));

    let exports = table.module_exports(1);
    assert_eq!(exports.len(), 2);

    let has_10 = exports.iter().any(|(f, a, _)| *f == 10 && *a == 0);
    let has_20 = exports.iter().any(|(f, a, _)| *f == 20 && *a == 1);
    assert!(has_10);
    assert!(has_20);
}

#[test]
fn test_export_table_module_exports_empty() {
    let table = ExportTable::new();
    table.register(1, 10, 0, CodePtr::null());

    let exports = table.module_exports(99);
    assert!(exports.is_empty());
}

#[test]
fn test_export_table_overwrite_existing() {
    let table = ExportTable::new();
    let ptr1 = CodePtr::from_raw(0x1000);
    let ptr2 = CodePtr::from_raw(0x2000);

    table.register(1, 1, 0, ptr1);
    assert_eq!(table.lookup(1, 1, 0).unwrap(), ptr1);

    table.register(1, 1, 0, ptr2);
    assert_eq!(table.lookup(1, 1, 0).unwrap(), ptr2);
    assert_eq!(table.len(), 1);
}

// ============================================================
// LazyFnRef tests
// ============================================================

#[test]
fn test_lazy_fn_ref_new() {
    let lazy = LazyFnRef::new(1, 2, 3);
    assert!(!lazy.is_resolved());
}

#[test]
fn test_lazy_fn_ref_get_initial() {
    let lazy = LazyFnRef::new(1, 2, 3);
    assert!(lazy.get().is_null());
}

#[test]
fn test_lazy_fn_ref_set_and_get() {
    let lazy = LazyFnRef::new(1, 2, 3);
    let ptr = CodePtr::from_raw(0xDEAD);
    lazy.set(ptr);
    assert_eq!(lazy.get(), ptr);
}

#[test]
fn test_lazy_fn_ref_is_resolved_false() {
    let lazy = LazyFnRef::new(1, 2, 3);
    assert!(!lazy.is_resolved());
}

#[test]
fn test_lazy_fn_ref_is_resolved_true() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::from_raw(0xBEEF));
    assert!(lazy.is_resolved());
}

#[test]
fn test_lazy_fn_ref_set_null() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::null());
    assert!(!lazy.is_resolved());
    assert!(lazy.get().is_null());
}

#[test]
fn test_lazy_fn_ref_set_multiple_times() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::from_raw(0x1000));
    assert_eq!(lazy.get(), CodePtr::from_raw(0x1000));

    lazy.set(CodePtr::from_raw(0x2000));
    assert_eq!(lazy.get(), CodePtr::from_raw(0x2000));

    lazy.set(CodePtr::null());
    assert!(lazy.get().is_null());
    assert!(!lazy.is_resolved());
}

#[test]
fn test_lazy_fn_ref_clone() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::from_raw(0xCAFE));

    let cloned = lazy.clone();
    assert_eq!(cloned.get(), CodePtr::from_raw(0xCAFE));
    assert!(cloned.is_resolved());
}

#[test]
fn test_lazy_fn_ref_clone_independence() {
    let lazy = LazyFnRef::new(1, 2, 3);
    lazy.set(CodePtr::from_raw(0x1000));

    let cloned = lazy.clone();

    lazy.set(CodePtr::from_raw(0x2000));
    assert_eq!(lazy.get(), CodePtr::from_raw(0x2000));
    assert_eq!(cloned.get(), CodePtr::from_raw(0x1000));
}

// ============================================================
// HotCodeManager tests
// ============================================================

#[test]
fn test_hot_code_manager_new() {
    let _mgr = HotCodeManager::new();
}

#[test]
fn test_hot_code_manager_default() {
    let _mgr = HotCodeManager::default();
}

#[test]
fn test_hot_code_manager_update_and_get_module() {
    let mgr = HotCodeManager::new();
    let module = CompiledModule {
        name: 42,
        exports: vec![],
        ir_module: IRModule::new(42),
        metadata: ModuleMetadata::default(),
    };

    mgr.update_module(42, module.clone());
    let retrieved = mgr.get_module(42);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, 42);
}

#[test]
fn test_hot_code_manager_get_module_missing() {
    let mgr = HotCodeManager::new();
    assert!(mgr.get_module(999).is_none());
}

#[test]
fn test_hot_code_manager_has_module_true() {
    let mgr = HotCodeManager::new();
    let module = CompiledModule {
        name: 10,
        exports: vec![],
        ir_module: IRModule::new(10),
        metadata: ModuleMetadata::default(),
    };

    mgr.update_module(10, module);
    assert!(mgr.has_module(10));
}

#[test]
fn test_hot_code_manager_has_module_false() {
    let mgr = HotCodeManager::new();
    assert!(!mgr.has_module(999));
}

#[test]
fn test_hot_code_manager_remove_module_success() {
    let mgr = HotCodeManager::new();
    let module = CompiledModule {
        name: 10,
        exports: vec![],
        ir_module: IRModule::new(10),
        metadata: ModuleMetadata::default(),
    };

    mgr.update_module(10, module);
    assert!(mgr.has_module(10));

    let removed = mgr.remove_module(10);
    assert!(removed);
    assert!(!mgr.has_module(10));
}

#[test]
fn test_hot_code_manager_remove_module_missing() {
    let mgr = HotCodeManager::new();
    let removed = mgr.remove_module(999);
    assert!(!removed);
}

#[test]
fn test_hot_code_manager_update_module_overwrite() {
    let mgr = HotCodeManager::new();

    let module1 = CompiledModule {
        name: 10,
        exports: vec![],
        ir_module: IRModule::new(10),
        metadata: ModuleMetadata::default(),
    };
    mgr.update_module(10, module1);
    assert!(mgr.has_module(10));

    let module2 = CompiledModule {
        name: 10,
        exports: vec![],
        ir_module: IRModule::new(10),
        metadata: ModuleMetadata {
            source_file: Some("new.beam".to_string()),
            ..Default::default()
        },
    };
    mgr.update_module(10, module2);

    let retrieved = mgr.get_module(10).unwrap();
    assert_eq!(retrieved.metadata.source_file, Some("new.beam".to_string()));
}

#[test]
fn test_hot_code_manager_update_module_definitions() {
    let mgr = HotCodeManager::new();
    mgr.update_module_definitions(42);
}

#[test]
fn test_hot_code_manager_update_module_with_exports() {
    let mgr = HotCodeManager::new();
    let ptr = CodePtr::from_raw(0x1000);

    let module = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 1,
            arity: 0,
            code_ptr: ptr,
            lazy_ref: LazyFnRef::new(1, 1, 0),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    mgr.update_module(1, module);
    assert!(mgr.has_module(1));
}

// ============================================================
// DispatchManager tests
// ============================================================

#[test]
fn test_dispatch_manager_new() {
    let _dm = DispatchManager::new();
}

#[test]
fn test_dispatch_manager_default() {
    let _dm = DispatchManager::default();
}

#[test]
fn test_dispatch_manager_register_module() {
    let dm = DispatchManager::new();
    let module = CompiledModule {
        name: 1,
        exports: vec![],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    let name = dm.register_module(module);
    assert_eq!(name, 1);
}

#[test]
fn test_dispatch_manager_register_module_with_exports() {
    let dm = DispatchManager::new();
    let ptr = CodePtr::from_raw(0x1000);
    let module = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: ptr,
            lazy_ref: LazyFnRef::new(1, 10, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    dm.register_module(module);

    let result = dm.lookup_function(1, 10, 2);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), ptr);
}

#[test]
fn test_dispatch_manager_lookup_function_hit() {
    let dm = DispatchManager::new();
    let ptr = CodePtr::from_raw(0xABCD);
    let module = CompiledModule {
        name: 5,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: ptr,
            lazy_ref: LazyFnRef::new(5, 10, 2),
        }],
        ir_module: IRModule::new(5),
        metadata: ModuleMetadata::default(),
    };

    dm.register_module(module);

    let result = dm.lookup_function(5, 10, 2);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), ptr);
}

#[test]
fn test_dispatch_manager_lookup_function_miss() {
    let dm = DispatchManager::new();
    let module = CompiledModule {
        name: 5,
        exports: vec![],
        ir_module: IRModule::new(5),
        metadata: ModuleMetadata::default(),
    };

    dm.register_module(module);

    assert!(dm.lookup_function(5, 10, 2).is_none());
}

#[test]
fn test_dispatch_manager_lookup_function_miss_empty() {
    let dm = DispatchManager::new();
    assert!(dm.lookup_function(1, 1, 1).is_none());
}

#[test]
fn test_dispatch_manager_hot_replace_success() {
    let dm = DispatchManager::new();
    let ptr1 = CodePtr::from_raw(0x1000);
    let module1 = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: ptr1,
            lazy_ref: LazyFnRef::new(1, 10, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    dm.register_module(module1);

    let ptr2 = CodePtr::from_raw(0x2000);
    let module2 = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: ptr2,
            lazy_ref: LazyFnRef::new(1, 10, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    let result = dm.hot_replace(module2);
    assert!(result.is_ok());
}

#[test]
fn test_dispatch_manager_hot_replace_mismatch() {
    let dm = DispatchManager::new();
    let module1 = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: CodePtr::from_raw(0x1000),
            lazy_ref: LazyFnRef::new(1, 10, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    dm.register_module(module1);

    let module2 = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 99,
            arity: 2,
            code_ptr: CodePtr::from_raw(0x2000),
            lazy_ref: LazyFnRef::new(1, 99, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    let result = dm.hot_replace(module2);
    assert!(result.is_err());
}

#[test]
fn test_dispatch_manager_hot_replace_new_module() {
    let dm = DispatchManager::new();

    let module = CompiledModule {
        name: 1,
        exports: vec![ExportEntry {
            function: 10,
            arity: 2,
            code_ptr: CodePtr::from_raw(0x1000),
            lazy_ref: LazyFnRef::new(1, 10, 2),
        }],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };

    let result = dm.hot_replace(module);
    assert!(result.is_ok());
}

#[test]
fn test_dispatch_manager_code_registry() {
    let dm = DispatchManager::new();
    let _registry = dm.code_registry();
}

// ============================================================
// HotCodeError tests
// ============================================================

#[test]
fn test_hot_code_error_display_export_mismatch() {
    let err = HotCodeError::ExportMismatch;
    let msg = format!("{}", err);
    assert_eq!(msg, "export mismatch - cannot hot-replace module");
}

#[test]
fn test_hot_code_error_display_module_not_found() {
    let err = HotCodeError::ModuleNotFound(42);
    let msg = format!("{}", err);
    assert_eq!(msg, "module not found: 42");
}

#[test]
fn test_hot_code_error_display_compilation_error() {
    let err = HotCodeError::CompilationError("something went wrong".to_string());
    let msg = format!("{}", err);
    assert_eq!(msg, "compilation error: something went wrong");
}

// ============================================================
// CompiledModule tests
// ============================================================

#[test]
fn test_compiled_module_empty_exports() {
    let module = CompiledModule {
        name: 1,
        exports: vec![],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };
    assert_eq!(module.name, 1);
    assert!(module.exports.is_empty());
}

#[test]
fn test_compiled_module_with_exports() {
    let module = CompiledModule {
        name: 1,
        exports: vec![
            ExportEntry {
                function: 1,
                arity: 0,
                code_ptr: CodePtr::from_raw(0x1000),
                lazy_ref: LazyFnRef::new(1, 1, 0),
            },
            ExportEntry {
                function: 2,
                arity: 1,
                code_ptr: CodePtr::from_raw(0x2000),
                lazy_ref: LazyFnRef::new(1, 2, 1),
            },
        ],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };
    assert_eq!(module.exports.len(), 2);
}

#[test]
fn test_compiled_module_clone() {
    let module = CompiledModule {
        name: 1,
        exports: vec![],
        ir_module: IRModule::new(1),
        metadata: ModuleMetadata::default(),
    };
    let cloned = module.clone();
    assert_eq!(cloned.name, module.name);
}

// ============================================================
// ModuleMetadata tests
// ============================================================

#[test]
fn test_module_metadata_default() {
    let meta = ModuleMetadata::default();
    assert!(meta.source_file.is_none());
    assert!(meta.compiler_options.is_empty());
    assert_eq!(meta.code_size, 0);
}

#[test]
fn test_module_metadata_custom() {
    let meta = ModuleMetadata {
        source_file: Some("test.beam".to_string()),
        compiler_options: vec!["debug_info".to_string(), "inline".to_string()],
        code_size: 1024,
    };
    assert_eq!(meta.source_file, Some("test.beam".to_string()));
    assert_eq!(meta.compiler_options.len(), 2);
    assert_eq!(meta.code_size, 1024);
}

#[test]
fn test_module_metadata_clone() {
    let meta = ModuleMetadata {
        source_file: Some("test.beam".to_string()),
        compiler_options: vec!["debug_info".to_string()],
        code_size: 512,
    };
    let cloned = meta.clone();
    assert_eq!(cloned.source_file, meta.source_file);
    assert_eq!(cloned.compiler_options, meta.compiler_options);
    assert_eq!(cloned.code_size, meta.code_size);
}
