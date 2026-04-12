use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use std::slice;

mod bloom_filter;
mod trie;

use trie::Trie;

/// # Safety
///
/// This function is unsafe because it returns a raw pointer that must be managed by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_new(size: usize, num_hashes: usize) -> *mut Trie {
    Box::into_raw(Box::new(Trie::new(size, num_hashes)))
}

/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that the `trie` pointer is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_free(trie: *mut Trie) {
    if !trie.is_null() {
        let _ = Box::from_raw(trie);
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `word` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_insert(trie: *mut Trie, word: *const c_char) {
    if trie.is_null() || word.is_null() { return; }
    let c_str = CStr::from_ptr(word);
    if let Ok(word_str) = c_str.to_str() {
        (*trie).insert(word_str);
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `word` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_contains(trie: *const Trie, word: *const c_char) -> bool {
    if trie.is_null() || word.is_null() { return false; }
    let c_str = CStr::from_ptr(word);
    match c_str.to_str() {
        Ok(word_str) => (*trie).contains(word_str),
        Err(_) => false,
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that the `trie` pointer is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_debug_print(trie: *const Trie) {
    if !trie.is_null() {
        (*trie).print();
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that `trie`, `prefix`, and `out_len` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_words_with_prefix(
    trie: *const Trie,
    prefix: *const c_char,
    out_len: *mut usize,
) -> *mut *mut c_char {
    if trie.is_null() || prefix.is_null() || out_len.is_null() {
        return ptr::null_mut();
    }

    let c_str = CStr::from_ptr(prefix);
    let words = match c_str.to_str() {
        Ok(prefix_str) => (*trie).words_with_prefix(prefix_str),
        Err(_) => Vec::new(),
    };

    *out_len = words.len();
    if words.is_empty() {
        return ptr::null_mut();
    }

    let mut c_strings = Vec::with_capacity(words.len());
    for word in words {
        if let Ok(c_str) = CString::new(word) {
            c_strings.push(c_str.into_raw());
        }
    }

    if c_strings.is_empty() {
        return ptr::null_mut();
    }

    let array_ptr = c_strings.as_mut_ptr();
    std::mem::forget(c_strings);
    array_ptr
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that `words` is a valid pointer to an array of `len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_free_words(words: *mut *mut c_char, len: usize) {
    if words.is_null() || len == 0 { return; }
    let slice = slice::from_raw_parts(words, len);
    for &ptr in slice {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
    let _ = Vec::from_raw_parts(words, len, len);
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `words` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_bulk_insert(trie: *mut Trie, words: *const *const c_char, len: usize) {
    if trie.is_null() || words.is_null() || len == 0 { return; }
    let slice = slice::from_raw_parts(words, len);
    let trie = &mut *trie;
    #[allow(clippy::collapsible_if)]
    for &word_ptr in slice {
        if !word_ptr.is_null() {
            if let Ok(word) = CStr::from_ptr(word_ptr).to_str() {
                trie.insert(word);
            }
        }
    }
}
