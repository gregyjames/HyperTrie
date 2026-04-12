using System.Buffers;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace HyperTrieCore;
public class TrieNative(int size, int numHashes) : IDisposable
{
    private readonly IntPtr _handle = trie_new(size, numHashes);

    private const string DllName = "hypertrie";

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr trie_new(int size, int numHashes);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_free(IntPtr trie);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_insert(IntPtr trie, IntPtr word);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool trie_contains(IntPtr trie, IntPtr word);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_debug_print(IntPtr trie);
    
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_free_words(IntPtr words, UIntPtr len);
    
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr trie_words_with_prefix(
        IntPtr trie,
        IntPtr prefix,
        // ReSharper disable once InconsistentNaming
        out UIntPtr out_len);
    
    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void trie_bulk_insert(IntPtr trie, IntPtr words, UIntPtr len);
    public void Insert(string word)
    {
        using var wordPtr = new Utf8String(word);
        trie_insert(_handle, wordPtr.Pointer);
    }

    public List<string> GetWordsWithPrefix(string prefix)
    {
        var result = new List<string>();
        
        using var prefixPtr = new Utf8String(prefix);
        var wordsPtr = trie_words_with_prefix(_handle, prefixPtr.Pointer, out var len);
        var length = len.ToUInt64();

        if (wordsPtr != IntPtr.Zero && length > 0)
        {
            IntPtr[] stringPtrs = new IntPtr[length];
            Marshal.Copy(wordsPtr, stringPtrs, 0, (int)length);

            for (ulong i = 0; i < length; i++)
            {
                string? word = Marshal.PtrToStringUTF8(stringPtrs[i]);
                if (word != null) result.Add(word);
            }

            // Free the words array & strings
            trie_free_words(wordsPtr, len);
        }
        
        return result;
    }

    public void Print()
    {
        trie_debug_print(_handle);
    }

    public bool Contains(string word)
    {
        using var testWord = new Utf8String(word);
        return trie_contains(_handle, testWord.Pointer);
    }
    
    public unsafe void BulkInsert(List<string> words)
    {
        // Materialize words once
        int count = words.Count;
        if (count == 0) return;
        
        int totalByteCapacity = 0;
        foreach (var word in words) totalByteCapacity += (word.Length * 3) + 1; // Worst case UTF8

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
                if (s == null) continue;
                
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
    
    private void ReleaseUnmanagedResources()
    {
        trie_free(_handle);
    }

    public void Dispose()
    {
        ReleaseUnmanagedResources();
        GC.SuppressFinalize(this);
    }

    ~TrieNative()
    {
        ReleaseUnmanagedResources();
    }
}