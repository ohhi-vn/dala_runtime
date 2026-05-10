//! Copying garbage collector - semi-space copying collection.
//!
//! This implements the classic Cheney semi-space copying collector,
//! adapted for the BEAM process model. Live objects are copied from
//! the old heap to a new heap, and all references are updated.

use crate::term::Term;
use crate::term::tags;
use crate::process::Process;
use crate::gc::rootset::RootSet;

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
    rootset: &RootSet,
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
    for &root_ptr in &rootset.stack_roots {
        let term = *root_ptr;
        if term.is_boxed() || term.is_list() {
            let new_addr = copy_object(root_ptr, &mut scan_ptr, new_heap);
            // Update the root pointer
            std::ptr::write(root_ptr as *mut Term, Term::from_raw(new_addr));
        }
    }

    // Copy register roots
    for &root_ptr in &rootset.register_roots {
        let term = *root_ptr;
        if term.is_boxed() || term.is_list() {
            let new_addr = copy_object(root_ptr, &mut scan_ptr, new_heap);
            std::ptr::write(root_ptr as *mut Term, Term::from_raw(new_addr));
        }
    }

    // Copy catch stack roots
    for &root_ptr in &rootset.catch_roots {
        let term = *root_ptr;
        if term.is_boxed() || term.is_list() {
            let new_addr = copy_object(root_ptr, &mut scan_ptr, new_heap);
            std::ptr::write(root_ptr as *mut Term, Term::from_raw(new_addr));
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
    let old_size = process.heap_top as usize - old_heap_start as usize;
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
    // If the pointer is within the new heap, it's already been copied
    if ptr >= new_heap && ptr < *scan_ptr {
        // Already copied - read the forwarding address
        let header = *ptr;
        if header.to_raw() & 0x1 == 1 {
            // This is a forwarding pointer (odd tagged)
            return header.to_raw() - 1;
        }
    }

    // Copy the object based on its type
    let (size, new_addr) = match if term.is_list() {
        // Cons cell: 2 words (head, tail)
        let new_addr = *scan_ptr;
        **scan_ptr = *ptr;          // head
        *(*scan_ptr).add(1) = *ptr.add(1); // tail
        *scan_ptr = scan_ptr.add(2);
        (2, new_addr)
    } else {
        // Boxed value: header + arity words
        let header = *ptr;
        let arity = Term::header_arity(header);
        let total_words = 1 + arity; // header + data
        let new_addr = *scan_ptr;

        // Copy all words
        for i in 0..total_words {
            *scan_ptr.add(i) = *ptr.add(i);
        }
        *scan_ptr = scan_ptr.add(total_words);
        (total_words, new_addr)
    };

    // Write forwarding pointer at the old location
    // Use odd tag to distinguish from normal headers
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
        if ptr >= base {
            // Internal pointer - needs updating
            let new_ptr = (ptr as usize + offset) as *const Term;
            *term = Term::from_raw(new_ptr as u64 | tags::PRIMARY_TAG_BOXED);
        }
    } else if term.is_list() {
        let ptr = term.get_list_ptr();
        if ptr >= base {
            let new_ptr = (ptr as usize + offset) as *const Term;
            *term = Term::from_raw(new_ptr as u64 | tags::PRIMARY_TAG_LIST);
        }
    }
}
