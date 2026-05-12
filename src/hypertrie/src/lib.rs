use std::ffi::CString;
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
pub unsafe extern "C" fn trie_insert(trie: *mut Trie, word: *const u8, len: usize) {
    if trie.is_null() || word.is_null() {
        return;
    }
    unsafe {
        let word_slice = slice::from_raw_parts(word, len);
        let word_str = std::str::from_utf8_unchecked(word_slice);
        (*trie).insert(word_str);
    }
}

/// # Safety
///
/// This function is unsafe because it dereferences raw pointers.
/// The caller must ensure that both `trie` and `word` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_contains(trie: *const Trie, word: *const u8, len: usize) -> bool {
    if trie.is_null() || word.is_null() {
        return false;
    }
    unsafe {
        let word_slice = slice::from_raw_parts(word, len);
        let word_str = std::str::from_utf8_unchecked(word_slice);
        (*trie).contains(word_str)
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
    prefix: *const u8,
    len: usize,
    out_len: *mut usize,
) -> *mut *mut c_char {
    if trie.is_null() || prefix.is_null() || out_len.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let prefix_slice = slice::from_raw_parts(prefix, len);
        let prefix_str = std::str::from_utf8_unchecked(prefix_slice);
        let words = (*trie).words_with_prefix(prefix_str);

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
/// The caller must ensure that `trie`, `words`, and `lens` are valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trie_bulk_insert(
    trie: *mut Trie,
    words: *const *const u8,
    lens: *const usize,
    count: usize,
) {
    if trie.is_null() || words.is_null() || lens.is_null() || count == 0 {
        return;
    }
    unsafe {
        let words_slice = slice::from_raw_parts(words, count);
        let lens_slice = slice::from_raw_parts(lens, count);
        let trie = &mut *trie;
        for i in 0..count {
            let word_ptr = words_slice[i];
            let word_len = lens_slice[i];
            if !word_ptr.is_null() && word_len > 0 {
                let word_bytes = slice::from_raw_parts(word_ptr, word_len);
                let word = std::str::from_utf8_unchecked(word_bytes);
                trie.insert(word);
            }
        }
        trie.shrink();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::ptr;

    fn make_trie() -> *mut Trie {
        unsafe { trie_new(1024, 3) }
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
        let word = "hello";
        unsafe {
            trie_insert(t, word.as_ptr(), word.len());
            assert!(trie_contains(t, word.as_ptr(), word.len()));
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_contains_missing_word_returns_false() {
        let t = make_trie();
        let word = "ghost";
        unsafe {
            assert!(!trie_contains(t, word.as_ptr(), word.len()));
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_insert_null_trie_is_safe() {
        let word = "hello";
        unsafe { trie_insert(ptr::null_mut(), word.as_ptr(), word.len()) };
    }

    #[test]
    fn test_insert_null_word_is_safe() {
        let t = make_trie();
        unsafe { trie_insert(t, ptr::null(), 0) };
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_contains_null_trie_returns_false() {
        let word = "hello";
        let result = unsafe { trie_contains(ptr::null(), word.as_ptr(), word.len()) };
        assert!(!result);
    }

    #[test]
    fn test_contains_null_word_returns_false() {
        let t = make_trie();
        let result = unsafe { trie_contains(t, ptr::null(), 0) };
        assert!(!result);
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_insert_empty_string() {
        let t = make_trie();
        let word = "";
        unsafe {
            trie_insert(t, word.as_ptr(), word.len());
            // whether empty string is "found" is implementation-defined;
            // the important thing is it doesn't crash
            let _ = trie_contains(t, word.as_ptr(), word.len());
        }
        unsafe { trie_free(t) };
    }

    // --- trie_bulk_insert ---

    #[test]
    fn test_bulk_insert_and_contains() {
        let t = make_trie();
        let words = ["foo", "bar", "baz"];
        let ptrs: Vec<*const u8> = words.iter().map(|s| s.as_ptr()).collect();
        let lens: Vec<usize> = words.iter().map(|s| s.len()).collect();
        unsafe {
            trie_bulk_insert(t, ptrs.as_ptr(), lens.as_ptr(), ptrs.len());
            for w in &words {
                assert!(trie_contains(t, w.as_ptr(), w.len()));
            }
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_bulk_insert_null_trie_is_safe() {
        let word = "foo";
        let ptrs = [word.as_ptr()];
        let lens = [word.len()];
        unsafe { trie_bulk_insert(ptr::null_mut(), ptrs.as_ptr(), lens.as_ptr(), 1) };
    }

    #[test]
    fn test_bulk_insert_null_words_is_safe() {
        let t = make_trie();
        unsafe { trie_bulk_insert(t, ptr::null(), ptr::null(), 0) };
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_bulk_insert_zero_len_is_safe() {
        let t = make_trie();
        let word = "foo";
        let ptrs = [word.as_ptr()];
        let lens = [word.len()];
        unsafe { trie_bulk_insert(t, ptrs.as_ptr(), lens.as_ptr(), 0) };
        unsafe { trie_free(t) };
    }

    // --- trie_words_with_prefix ---

    #[test]
    fn test_words_with_prefix_basic() {
        let t = make_trie();
        let words = ["apple", "application", "apply", "banana"];
        let ptrs: Vec<*const u8> = words.iter().map(|s| s.as_ptr()).collect();
        let lens: Vec<usize> = words.iter().map(|s| s.len()).collect();
        unsafe {
            trie_bulk_insert(t, ptrs.as_ptr(), lens.as_ptr(), ptrs.len());

            let prefix = "app";
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), prefix.len(), &mut out_len);

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
        let word = "hello";
        unsafe {
            trie_insert(t, word.as_ptr(), word.len());

            let prefix = "xyz";
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), prefix.len(), &mut out_len);

            assert!(result.is_null());
            assert_eq!(out_len, 0);
        }
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_words_with_prefix_null_trie_returns_null() {
        let prefix = "app";
        let mut out_len: usize = 0;
        let result = unsafe {
            trie_words_with_prefix(ptr::null(), prefix.as_ptr(), prefix.len(), &mut out_len)
        };
        assert!(result.is_null());
    }

    #[test]
    fn test_words_with_prefix_null_prefix_returns_null() {
        let t = make_trie();
        let mut out_len: usize = 0;
        let result = unsafe { trie_words_with_prefix(t, ptr::null(), 0, &mut out_len) };
        assert!(result.is_null());
        unsafe { trie_free(t) };
    }

    #[test]
    fn test_words_with_prefix_null_out_len_returns_null() {
        let t = make_trie();
        let prefix = "app";
        let result =
            unsafe { trie_words_with_prefix(t, prefix.as_ptr(), prefix.len(), ptr::null_mut()) };
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
        let word = "hello";
        unsafe {
            trie_insert(t, word.as_ptr(), word.len());
            let prefix = "hel";
            let mut out_len: usize = 0;
            let result = trie_words_with_prefix(t, prefix.as_ptr(), prefix.len(), &mut out_len);
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
        let word = "test";
        unsafe {
            trie_insert(t, word.as_ptr(), word.len());
            trie_debug_print(t);
        }
        unsafe { trie_free(t) };
    }
}
