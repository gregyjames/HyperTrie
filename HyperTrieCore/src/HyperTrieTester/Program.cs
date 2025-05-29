using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Gma.DataStructures.StringSearch;

namespace HyperTrieCore;

class TrieNative(int size, int numHashes) : IDisposable
{
    private readonly IntPtr _handle = trie_new(size, numHashes);

    private const string DllName = "libhypertrie";

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
    private static extern void trie_bulk_insert(IntPtr trie, IntPtr[] words, UIntPtr len);
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
        
        // Calculate total buffer size for all UTF8 strings + null terminators
        var totalSize = 0;
        var offsets = new List<int>(count);

        foreach (var word in words)
        {
            offsets.Add(totalSize);
            var byteCount = Encoding.UTF8.GetByteCount(word);
            totalSize += byteCount + 1; // +1 for null terminator
        }

        // Allocate one big unmanaged buffer
        IntPtr bigBuffer = Marshal.AllocHGlobal(totalSize);

        // Allocate managed pointer array
        IntPtr[] ptrArray = new IntPtr[count];

        try
        {
            byte* basePtr = (byte*)bigBuffer.ToPointer();

            /*
            for (int i = 0; i < count; i++)
            {
                var sourceSpan = wordList[i].AsSpan();
                var offset = offsets[i];
                var destSpan = new Span<byte>(basePtr + offset, totalSize - offset);

                int bytesEncoded = Encoding.UTF8.GetBytes(sourceSpan, destSpan);
                destSpan[bytesEncoded] = 0; // null terminator
                ptrArray[i] = (IntPtr)(basePtr + offset);
            }*/

            var collection = CollectionsMarshal.AsSpan(words);
            ref var searchSpace = ref MemoryMarshal.GetReference(collection);
            
            for (int i = 0; i < count; i++)
            {
                var item = Unsafe.Add(ref searchSpace, i).AsSpan();
                var offset = offsets[i];
                var destSpan = new Span<byte>(basePtr + offset, totalSize - offset);

                int bytesEncoded = Encoding.UTF8.GetBytes(item, destSpan);
                destSpan[bytesEncoded] = 0; // null terminator
                ptrArray[i] = (IntPtr)(basePtr + offset);
            }
            
            // Call Rust bulk insert once
            trie_bulk_insert(_handle, ptrArray, (UIntPtr)ptrArray.Length);
        }
        finally
        {
            // Free single big buffer
            Marshal.FreeHGlobal(bigBuffer);
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
    
    /// <summary>
    /// Helper to marshal C# string to UTF8 IntPtr with disposal.
    /// </summary>
    private class Utf8String : IDisposable
    {
        public IntPtr Pointer { get; private set; }

        public Utf8String(string str)
        {
            if (str == null) throw new ArgumentNullException(nameof(str));

            // Get byte array directly, with null terminator
            byte[] utf8Bytes = Encoding.UTF8.GetBytes(str);
            int lengthWithNullTerminator = utf8Bytes.Length + 1;  // +1 for the null terminator

            // Allocate memory and copy bytes to unmanaged memory
            Pointer = Marshal.AllocHGlobal(lengthWithNullTerminator);
            Marshal.Copy(utf8Bytes, 0, Pointer, utf8Bytes.Length);
            Marshal.WriteByte(Pointer + utf8Bytes.Length, 0); 
        }
        
        private bool _disposed;

        public void Dispose()
        {
            if (_disposed) return;

            if (Pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(Pointer);
                Pointer = IntPtr.Zero;
            }

            _disposed = true;
            GC.SuppressFinalize(this);
        }
        
        ~Utf8String()
        {
            Dispose();
        }
    }
}

internal static class Program
{
    public static void Main(string[] args)
    {
        const string url = "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt";
        const int numTries = 500;
        
        var client = new HttpClient();
        var content = client.GetStringAsync(url).Result;
        var allWords = content.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).Select(x => x.ToLower().Trim()).ToList();
        var trieNative = new TrieNative(allWords.Count(), 3);
        var trieCSharp = new Trie<string>();
        
        Random random = new Random();
        var sw = Stopwatch.StartNew();
        foreach (var word in allWords)
        {
            trieCSharp.Add(word, word);
        }
        
        for (int i = 0; i < numTries; i++)
        {
            var indx = random.Next(0, allWords.Count());
            trieCSharp.Retrieve(indx % 2 == 0 ? allWords[indx] : string.Join("", allWords[indx].Reverse()));
        }
        sw.Stop();
        
        Console.WriteLine($"Took {sw.ElapsedMilliseconds}ms to run Trie.Net");
        
        sw.Restart();
        trieNative.BulkInsert(allWords);
        for (int i = 0; i < numTries; i++)
        {
            var indx = random.Next(0, allWords.Count());
            trieNative.Contains(indx % 2 == 0 ? allWords[indx] : string.Join("", allWords[indx].Reverse()));
        }
        sw.Stop();
        
        Console.WriteLine($"Took {sw.ElapsedMilliseconds}ms to run Trie Native");
    }
}