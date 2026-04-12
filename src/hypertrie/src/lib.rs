use std::ffi::{CStr, CString};
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
        unsafe {
            let _ = Box::from_raw(trie);
        }
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `word` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_insert(trie: *mut Trie, word: *const c_char) {
    if trie.is_null() || word.is_null() {
        return;
    }
    unsafe {
        let c_str = CStr::from_ptr(word);
        if let Ok(word_str) = c_str.to_str() {
            (*trie).insert(word_str);
        }
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `word` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_contains(trie: *const Trie, word: *const c_char) -> bool {
    if trie.is_null() || word.is_null() {
        return false;
    }
    unsafe {
        let c_str = CStr::from_ptr(word);
        match c_str.to_str() {
            Ok(word_str) => (*trie).contains(word_str),
            Err(_) => false,
        }
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that the `trie` pointer is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_debug_print(trie: *const Trie) {
    if !trie.is_null() {
        unsafe {
            (*trie).print();
        }
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

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that `words` is a valid pointer to an array of `len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_free_words(words: *mut *mut c_char, len: usize) {
    if words.is_null() || len == 0 {
        return;
    }
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

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `words` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_bulk_insert(
    trie: *mut Trie,
    words: *const *const c_char,
    len: usize,
) {
    if trie.is_null() || words.is_null() || len == 0 {
        return;
    }
    unsafe {
        let slice = slice::from_raw_parts(words, len);
        let trie = &mut *trie;
        #[allow(clippy::collapsible_if)]
        for &word_ptr in slice {
            if !word_ptr.is_null() {
                let word = unsafe { 
                    std::str::from_utf8_unchecked(CStr::from_ptr(word_ptr).to_bytes()) 
                };
                trie.insert(word);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    fn make_trie() -> *mut Trie {
        unsafe { trie_new(1024, 3) }
    }

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    // --- trie_new / trie_free ---

    #[test]
    fn test_new_returns_non_null() {
        let t = make_trie();
        assert!(!t.is_null());
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_free_null_is_safe() {
        unsafe { trie_free(ptr::null_mut()) };
    }

    // --- trie_insert / trie_contains ---

    #[test]
    fn test_insert_and_contains() {
        let t = make_trie();
        let word = cstr("hello");
        unsafe {
            trie_insert(t, word.as_ptr());
            assert!(trie_contains(t, word.as_ptr()));
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_contains_missing_word_returns_false() {
        let t = make_trie();
        let word = cstr("ghost");
        unsafe {
            assert!(!trie_contains(t, word.as_ptr()));
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_insert_null_trie_is_safe() {
        let word = cstr("hello");
        unsafe { trie_insert(ptr::null_mut(), word.as_ptr()) };
    }

    #[test]
    fn test_insert_null_word_is_safe() {
        let t = make_trie();
        unsafe { trie_insert(t, ptr::null()) };
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_contains_null_trie_returns_false() {
        let word = cstr("hello");
        let result = unsafe { trie_contains(ptr::null(), word.as_ptr()) };
        assert!(!result);
    }

    #[test]
    fn test_contains_null_word_returns_false() {
        let t = make_trie();
        let result = unsafe { trie_contains(t, ptr::null()) };
        assert!(!result);
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_insert_empty_string() {
        let t = make_trie();
        let word = cstr("");
        unsafe {
            trie_insert(t, word.as_ptr());
            // whether empty string is "found" is implementation-defined;
            // the important thing is it doesn't crash
            let _ = trie_contains(t, word.as_ptr());
        }
        unsafe { trie_free(t) };
    }

    // --- trie_bulk_insert ---

    #[test]
    fn test_bulk_insert_and_contains() {
        let t = make_trie();
        let words = [cstr("foo"), cstr("bar"), cstr("baz")];
        let ptrs: Vec<*const c_char> = words.iter().map(|s| s.as_ptr()).collect();
        unsafe {
            trie_bulk_insert(t, ptrs.as_ptr(), ptrs.len());
            for w in &words {
                assert!(trie_contains(t, w.as_ptr()));
            }
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_bulk_insert_null_trie_is_safe() {
        let word = cstr("foo");
        let ptrs = [word.as_ptr()];
        unsafe { trie_bulk_insert(ptr::null_mut(), ptrs.as_ptr(), 1) };
    }

    #[test]
    fn test_bulk_insert_null_words_is_safe() {
        let t = make_trie();
        unsafe { trie_bulk_insert(t, ptr::null(), 0) };
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_bulk_insert_zero_len_is_safe() {
        let t = make_trie();
        let word = cstr("foo");
        let ptrs = [word.as_ptr()];
        unsafe { trie_bulk_insert(t, ptrs.as_ptr(), 0) };
        unsafe { trie_free(t) };
    }

    // --- trie_words_with_prefix ---

    #[test]
    fn test_words_with_prefix_basic() {
        let t = make_trie();
        let words = [
            cstr("apple"),
            cstr("application"),
            cstr("apply"),
            cstr("banana"),
        ];
        let ptrs: Vec<*const c_char> = words.iter().map(|s| s.as_ptr()).collect();
        unsafe {
            trie_bulk_insert(t, ptrs.as_ptr(), ptrs.len());

            let prefix = cstr("app");
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), &mut out_len);

            assert!(!result.is_null());
            assert_eq!(out_len, 3);

            // collect and verify all expected words are present
            let slice = slice::from_raw_parts(result, out_len);
            let found: Vec<String> = slice
                .iter()
                .map(|&p| CStr::from_ptr(p).to_str().unwrap().to_owned())
                .collect();

            assert!(found.contains(&"apple".to_string()));
            assert!(found.contains(&"application".to_string()));
            assert!(found.contains(&"apply".to_string()));

            trie_free_words(result, out_len);
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_words_with_prefix_no_match_returns_null() {
        let t = make_trie();
        let word = cstr("hello");
        unsafe {
            trie_insert(t, word.as_ptr());

            let prefix = cstr("xyz");
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), &mut out_len);

            assert!(result.is_null());
            assert_eq!(out_len, 0);
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_words_with_prefix_null_trie_returns_null() {
        let prefix = cstr("app");
        let mut out_len: usize = 0;
        let result = unsafe { trie_words_with_prefix(ptr::null(), prefix.as_ptr(), &mut out_len) };
        assert!(result.is_null());
    }

    #[test]
    fn test_words_with_prefix_null_prefix_returns_null() {
        let t = make_trie();
        let mut out_len: usize = 0;
        let result = unsafe { trie_words_with_prefix(t, ptr::null(), &mut out_len) };
        assert!(result.is_null());
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_words_with_prefix_null_out_len_returns_null() {
        let t = make_trie();
        let prefix = cstr("app");
        let result = unsafe { trie_words_with_prefix(t, prefix.as_ptr(), ptr::null_mut()) };
        assert!(result.is_null());
        unsafe { trie_free(t) };
    }

    // --- trie_free_words ---

    #[test]
    fn test_free_words_null_is_safe() {
        unsafe { trie_free_words(ptr::null_mut(), 0) };
    }

    #[test]
    fn test_free_words_zero_len_is_safe() {
        let t = make_trie();
        let word = cstr("hello");
        unsafe {
            trie_insert(t, word.as_ptr());
            let prefix = cstr("hel");
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), &mut out_len);
            // free with len=0 should be safe even with a non-null pointer
            trie_free_words(result, 0);
        }
        unsafe { trie_free(t) };
    }

    // --- trie_debug_print ---

    #[test]
    fn test_debug_print_null_is_safe() {
        unsafe { trie_debug_print(ptr::null()) };
    }

    #[test]
    fn test_debug_print_does_not_crash() {
        let t = make_trie();
        let word = cstr("test");
        unsafe {
            trie_insert(t, word.as_ptr());
            trie_debug_print(t);
        }
        unsafe { trie_free(t) };
    }
}
