use std::ffi::{CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use std::slice;

mod bloom_filter;
mod trie;

use trie::Trie;

#[unsafe(no_mangle)]
pub extern "C" fn trie_new(size: usize, num_hashes: usize) -> *mut Trie {
    Box::into_raw(Box::new(Trie::new(size, num_hashes)))
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_free(trie: *mut Trie) {
    if !trie.is_null() {
        unsafe { let _ = Box::from_raw(trie); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_insert(trie: *mut Trie, word: *const c_char) {
    if trie.is_null() || word.is_null() { return; }
    unsafe {
        let c_str = CStr::from_ptr(word);
        if let Ok(word_str) = c_str.to_str() {
            (*trie).insert(word_str);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_contains(trie: *const Trie, word: *const c_char) -> bool {
    if trie.is_null() || word.is_null() { return false; }
    unsafe {
        let c_str = CStr::from_ptr(word);
        match c_str.to_str() {
            Ok(word_str) => (*trie).contains(word_str),
            Err(_) => false,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_debug_print(trie: *const Trie) {
    if !trie.is_null() {
        unsafe { (*trie).print(); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_words_with_prefix(
    trie: *const Trie,
    prefix: *const c_char,
    out_len: *mut usize,
) -> *mut *mut c_char {
    if trie.is_null() || prefix.is_null() || out_len.is_null() {
        return ptr::null_mut();
    }

    unsafe {
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
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_free_words(words: *mut *mut c_char, len: usize) {
    if words.is_null() || len == 0 { return; }
    unsafe {
        let slice = slice::from_raw_parts(words, len);
        for &ptr in slice {
            if !ptr.is_null() {
                let _ = CString::from_raw(ptr);
            }
        }
        let _ = Vec::from_raw_parts(words, len, len);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trie_bulk_insert(trie: *mut Trie, words: *const *const c_char, len: usize) {
    if trie.is_null() || words.is_null() || len == 0 { return; }
    unsafe {
        let slice = slice::from_raw_parts(words, len);
        let trie = &mut *trie;
        for &word_ptr in slice {
            if !word_ptr.is_null() {
                if let Ok(word) = CStr::from_ptr(word_ptr).to_str() {
                    trie.insert(word);
                }
            }
        }
    }
}
