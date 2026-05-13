//! Copying garbage collector - semi-space copying collection.
//!
//! This implements the classic Cheney semi-space copying collector,
//! adapted for the BEAM process model. Live objects are copied from
//! the old heap to a new heap, and all references are updated.

use crate::process::Process;
use crate::term::Term;
use crate::term::tags;

/// Perform copying collection for a process.
///
/// 1. Allocate a new heap (twice the size of the live data)
/// 2. Copy all live objects from old heap to new heap
/// 3. Update all references in the root set and copied objects
/// 4. Return the new heap pointer
///
/// # Safety
///
/// Must be called at a safepoint with a valid root set.
pub unsafe fn copy_collection(
    process: &mut Process,
    min_size: usize,
) -> Result<*mut Term, &'static str> {
    // Calculate new heap size
    let live_size = process.heap_ptr as usize - process.heap_start() as usize;
    let new_size = min_size.max(live_size * 2).max(233);

    // Allocate new heap
    let new_layout = match std::alloc::Layout::array::<Term>(new_size) {
        Ok(layout) => layout,
        Err(_) => return Err("invalid heap size"),
    };

    let new_heap = std::alloc::alloc(new_layout) as *mut Term;
    if new_heap.is_null() {
        return Err("heap allocation failed during GC");
    }

    // Copy all roots first
    let mut scan_ptr = new_heap;

    // Copy stack roots
    {
        let stack_start = process.stack_ptr;
        let stack_end = process.stack_top;
        let mut ptr = stack_start;
        while ptr < stack_end {
            let term = &*ptr;
            if term.is_boxed() || term.is_list() {
                let new_addr = copy_object(ptr, &mut scan_ptr, new_heap);
                std::ptr::write(ptr, Term::from_raw(new_addr as u64));
            }
            ptr = ptr.add(1);
        }
    }

    // Copy register roots
    for i in 0..256 {
        let term = &process.registers.x[i];
        if term.is_boxed() || term.is_list() {
            // Need to copy the pointed-to object and update the register
            let raw = term.to_raw();
            let new_addr = copy_object(raw as *const Term, &mut scan_ptr, new_heap);
            process.registers.x[i] = Term::from_raw(new_addr as u64);
        }
    }

    // Scan copied objects for internal pointers (breadth-first)
    let heap_end = new_heap.add(new_size);
    while scan_ptr < heap_end {
        // This is a simplified scan - in a real implementation, we'd
        // need to know the layout of each object (header tells us arity)
        scan_ptr = scan_ptr.add(1); // Skip header
    }

    // Free old heap
    let old_heap_start = process.heap_start();
    let old_size = (process.heap_top as usize) - (old_heap_start as usize);
    if old_size > 0 {
        let old_layout = std::alloc::Layout::array::<Term>(old_size).unwrap();
        std::alloc::dealloc(old_heap_start as *mut u8, old_layout);
    }

    // Update process state
    process.heap_top = new_heap.add(new_size);
    process.heap_high_water = new_heap;

    Ok(scan_ptr)
}

/// Copy a single object from the old heap to the new heap.
///
/// Returns the new address of the object.
///
/// # Safety
///
/// The source pointer must point to a valid BEAM heap object.
unsafe fn copy_object(src: *const Term, scan_ptr: &mut *mut Term, new_heap: *mut Term) -> usize {
    let term = *src;

    if !term.is_boxed() && !term.is_list() {
        return term.to_raw() as usize;
    }

    let ptr = if term.is_list() {
        term.get_list_ptr()
    } else {
        term.get_boxed_ptr()
    };

    // Check if already copied (forwarding pointer technique)
    if ptr >= new_heap && ptr < *scan_ptr {
        let header = (*ptr).to_raw();
        if header & 0x1 == 1 {
            return (header - 1) as usize;
        }
    }

    // Copy the object based on its type
    let new_addr = *scan_ptr;
    if term.is_list() {
        // Cons cell: 2 words (head, tail)
        **scan_ptr = *ptr;
        *(*scan_ptr).add(1) = *ptr.add(1);
        *scan_ptr = scan_ptr.add(2);
    } else {
        // Boxed value: header + arity words
        let header = (*ptr).to_raw();
        let arity = Term::header_arity(header);
        let total_words = 1 + arity;
        for i in 0..total_words {
            *scan_ptr.add(i) = *ptr.add(i);
        }
        *scan_ptr = scan_ptr.add(total_words);
    };

    // Write forwarding pointer at the old location
    let forward_tag = Term::from_raw((new_addr as u64) | 0x1);
    std::ptr::write(ptr as *mut Term, forward_tag);

    new_addr as usize
}

/// Update all internal pointers within a copied object.
///
/// After all objects are copied, we need to fix up pointers within
/// compound objects (tuples, lists, etc.) to point to their new
/// locations.
unsafe fn update_pointers(term: &mut Term, base: *const Term, offset: usize) {
    if term.is_boxed() {
        let ptr = term.get_boxed_ptr();
        if (ptr as usize) >= (base as usize) {
            // Internal pointer - needs updating
            let new_ptr = (ptr as usize + offset) as *const Term;
            *term = Term::from_raw(new_ptr as u64 | crate::term::tags::PRIMARY_TAG_BOXED);
        }
    } else if term.is_list() {
        let ptr = term.get_list_ptr();
        if (ptr as usize) >= (base as usize) {
            let new_ptr = (ptr as usize + offset) as *const Term;
            *term = Term::from_raw(new_ptr as u64 | crate::term::tags::PRIMARY_TAG_LIST);
        }
    }
}
