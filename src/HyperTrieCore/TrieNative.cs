using System.Buffers;
using System.Runtime.InteropServices;
using System.Text;

namespace HyperTrieCore;
/// <summary>
/// Initialize a new TrieNative object.
/// </summary>
/// <param name="size">The maximum amount of words that will be held.</param>
/// <param name="numHashes">Number of times to hash in the BloomFilter.</param>
public sealed class TrieNative(int size, int numHashes = 5) : IDisposable
{
    private readonly IntPtr _handle = trie_new(size, numHashes);

    private const string DLL_NAME = "hypertrie";

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern IntPtr trie_new(int size, int numHashes);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern void trie_free(IntPtr trie);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern void trie_insert(IntPtr trie, IntPtr word);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool trie_contains(IntPtr trie, IntPtr word);
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_debug_print(IntPtr trie);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern void trie_free_words(IntPtr words, UIntPtr len);


    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern IntPtr trie_words_with_prefix(
        IntPtr trie,
        IntPtr prefix,
        // ReSharper disable once InconsistentNaming
        out UIntPtr out_len);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.SafeDirectories)]
    private static extern void trie_bulk_insert(IntPtr trie, IntPtr words, UIntPtr len);
    /// <summary>
    /// Inserts a new word into the TrieNative object.
    /// </summary>
    /// <param name="word">The word to insert.</param>
    public void Insert(string word)
    {
        using var wordPtr = new Utf8String(word);
        trie_insert(_handle, wordPtr.Pointer);
    }

    /// <summary>
    /// Returns a list of strings matching the specified prefix.
    /// </summary>
    /// <param name="prefix">The prefix to search for.</param>
    /// <returns></returns>
    public unsafe IEnumerable<string> GetWordsWithPrefix(string prefix)
    {
        var result = new List<string>();

        using var prefixPtr = new Utf8String(prefix);
        nint* wordsPtr = (IntPtr*)trie_words_with_prefix(_handle, prefixPtr.Pointer, out UIntPtr len);
        uint count = len.ToUInt32();

        if (wordsPtr == null || count == 0)
        {
            return [];
        }

        try
        {
            for (uint i = 0; i < count; i++)
            {
                IntPtr currentWordPtr = wordsPtr[i];

                if (currentWordPtr != IntPtr.Zero)
                {
                    string? word = Marshal.PtrToStringUTF8(currentWordPtr);
                    if (word != null)
                    {
                        result.Add(word);
                    }
                }
            }
        }
        finally
        {
            trie_free_words((IntPtr)wordsPtr, len);
        }

        return result;
    }

    /// <summary>
    /// Prints a representation of the TrieNative for debugging.
    /// </summary>
    public void Print() => trie_debug_print(_handle);

    /// <summary>
    /// Checks if the word exists in the TrieNative.
    /// </summary>
    /// <param name="word">The word to search for.</param>
    /// <returns></returns>
    public bool Contains(string word)
    {
        using var testWord = new Utf8String(word);
        return trie_contains(_handle, testWord.Pointer);
    }

    /// <summary>
    /// Bulk inserts a list of words into the TrieNative object.
    /// </summary>
    /// <param name="words">The list of words to insert.</param>
    public unsafe void BulkInsert(List<string>? words)
    {
        // Materialize words once
        if (words == null || words.Count == 0)
        {
            return;
        }

        int count = words.Count;
        if (count == 0){ return;}

        int totalByteCapacity = 0;
        foreach (string word in words)
        {
            totalByteCapacity += (word.Length * 3) + 1; // Worst case UTF8
        }

        IntPtr bigBuffer = Marshal.AllocHGlobal((IntPtr)totalByteCapacity);
        IntPtr[] ptrArray = ArrayPool<IntPtr>.Shared.Rent(count);


        try
        {
            byte* currentDest = (byte*)bigBuffer.ToPointer();

            #if NET5_0_OR_GREATER
                var span = CollectionsMarshal.AsSpan(words);
            #else
                var span = words.ToArray().AsSpan();
            #endif

            for (int i = 0; i < count; i++)
            {
                string s = span[i];
                if (string.IsNullOrEmpty(s))
                {
                    continue;
                }

                ptrArray[i] = (IntPtr)currentDest;

                fixed (char* pStr = s)
                {
                    int bytesWritten = Encoding.UTF8.GetBytes(pStr, s.Length, currentDest, totalByteCapacity);

                    currentDest += bytesWritten;
                    *currentDest = 0; // Null terminator
                    currentDest++;
                }
            }

            fixed (IntPtr* pPtrs = ptrArray)
            {
                trie_bulk_insert(_handle, (IntPtr)pPtrs, (UIntPtr)count);
            }
        }
        finally
        {
            Marshal.FreeHGlobal(bigBuffer);
            ArrayPool<IntPtr>.Shared.Return(ptrArray);
        }
    }

    private void ReleaseUnmanagedResources() => trie_free(_handle);

    /// <summary>
    /// Disposes the resources used by the TrieNative.
    /// </summary>
    public void Dispose()
    {
        ReleaseUnmanagedResources();
        GC.SuppressFinalize(this);
    }

    /// <summary>
    /// Finalizes the <see cref="TrieNative"/> instance, ensuring that the unmanaged
    /// Rust Trie memory is released if the object is collected by the Garbage Collector
    /// without being explicitly disposed.
    /// </summary>
    /// <remarks>
    /// This is a fallback mechanism. To ensure optimal memory management and
    /// immediate release of Rust resources, always call <see cref="Dispose()"/>
    /// or use a <c>using</c> block.
    /// </remarks>
    ~TrieNative() => ReleaseUnmanagedResources();
}
