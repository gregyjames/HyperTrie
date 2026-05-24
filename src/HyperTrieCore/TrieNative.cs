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
    private static extern IntPtr trie_new(int size, int numHashes);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_free(IntPtr trie);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_insert(IntPtr trie, IntPtr word, nuint len);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool trie_contains(IntPtr trie, IntPtr word, nuint len);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_debug_print(IntPtr trie);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_free_words(IntPtr words, nuint len);


    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr trie_words_with_prefix(
        IntPtr trie,
        IntPtr prefix,
        nuint prefix_len,
        // ReSharper disable once InconsistentNaming
        out nuint out_len);

    [DllImport(DLL_NAME, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_bulk_insert(IntPtr trie, IntPtr words, IntPtr word_lens, nuint len);
    /// <summary>
    /// Inserts a new word into the TrieNative object.
    /// </summary>
    /// <param name="word">The word to insert.</param>
    public unsafe void Insert(string word)
    {
        using var wordPtr = new Utf8String(word);
        trie_insert(_handle, wordPtr.Pointer, (nuint)wordPtr.Length);
    }

    /// <summary>
    /// Returns a list of strings matching the specified prefix.
    /// </summary>
    /// <param name="prefix">The prefix to search for.</param>
    /// <returns>An IEnumerable of matched words.</returns>
    public unsafe IEnumerable<string> GetWordsWithPrefix(string prefix)
    {
        var result = new List<string>();

        using var prefixPtr = new Utf8String(prefix);
        nint* wordsPtr = (IntPtr*)trie_words_with_prefix(_handle, prefixPtr.Pointer, (nuint)prefixPtr.Length, out nuint len);
        uint count = (uint)len;

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
    /// <returns>A bool representing if the word is found or not.</returns>
    public unsafe bool Contains(string word)
    {
        using var testWord = new Utf8String(word);
        return trie_contains(_handle, testWord.Pointer, (nuint)testWord.Length);
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

        long totalByteCapacity = words
            .Where(word => !string.IsNullOrEmpty(word))
            .Sum(word => (long)Encoding.UTF8.GetByteCount(word!) + 1);

        IntPtr bigBuffer = Marshal.AllocHGlobal(checked((IntPtr)totalByteCapacity));
        IntPtr[] ptrArray = ArrayPool<IntPtr>.Shared.Rent(count);
        nuint[] lenArray = ArrayPool<nuint>.Shared.Rent(count);

        try
        {
            byte* currentDest = (byte*)bigBuffer.ToPointer();
            int i = 0;
            foreach (string s in words)
            {
                if (string.IsNullOrEmpty(s))
                {
                    ptrArray[i] = IntPtr.Zero;
                    lenArray[i] = 0;
                    i++;
                    continue;
                }

                ptrArray[i] = (IntPtr)currentDest;
                int byteCount = Encoding.UTF8.GetByteCount(s);
                lenArray[i] = (nuint)byteCount;

                fixed (char* pStr = s)
                {
                    int bytesWritten = Encoding.UTF8.GetBytes(pStr, s.Length, currentDest, (int)(totalByteCapacity - (currentDest - (byte*)bigBuffer.ToPointer())));
                    currentDest += bytesWritten;
                    *currentDest = 0; // Null terminator
                    currentDest++;
                }
                i++;
            }

            fixed (IntPtr* pPtrs = ptrArray)
            fixed (nuint* pLens = lenArray)
            {
                trie_bulk_insert(_handle, (IntPtr)pPtrs, (IntPtr)pLens, (nuint)count);
            }
        }
        finally
        {
            Marshal.FreeHGlobal(bigBuffer);
            ArrayPool<IntPtr>.Shared.Return(ptrArray);
            ArrayPool<nuint>.Shared.Return(lenArray);
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
