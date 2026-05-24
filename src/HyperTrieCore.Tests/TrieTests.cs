using Xunit;
using HyperTrieCore;

namespace HyperTrieCore.Tests;

public class TrieTests
{
    [Fact]
    public void TestBasicInsertion()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("apple");
        trie.Insert("banana");

        Assert.True(trie.Contains("apple"));
        Assert.True(trie.Contains("banana"));
        Assert.False(trie.Contains("orange"));
    }
    
    [Fact]
    public void TestBulkInsertion()
    {
        using var trie = new TrieNative(4, 3);
        trie.BulkInsert(["apple", "banana", "", null!]);

        Assert.True(trie.Contains("apple"));
        Assert.True(trie.Contains("banana"));
        Assert.False(trie.Contains("orange"));
        Assert.False(trie.Contains(""));
    }

    [Fact]
    public void TestBulkInsertion_NullOrEmpty()
    {
        using var trie = new TrieNative(4, 3);
        trie.BulkInsert(null);
        trie.BulkInsert([]);

        Assert.False(trie.Contains("apple"));
    }

    [Fact]
    public void TestPrefixSearch()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("apple");
        trie.Insert("app");
        trie.Insert("application");
        
        var results = trie.GetWordsWithPrefix("app").ToList();
        
        Assert.Equal(3, results.Count);
        Assert.Contains("apple", results);
        Assert.Contains("app", results);
        Assert.Contains("application", results);
    }

    [Fact]
    public void TestPrefixSearch_NoMatch()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("apple");

        var results = trie.GetWordsWithPrefix("xyz");
        Assert.Empty(results);
    }

    [Fact]
    public void TestDebugPrint()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("a");
        // Just verify it doesn't crash
        trie.Print();
    }

    [Fact]
    public void TestDefaultNumHashes()
    {
        using var trie = new TrieNative(100);
        trie.Insert("test");
        Assert.True(trie.Contains("test"));
    }

    [Fact]
    public void TestInvalidCharactersIgnored()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("a1b"); // '1' should be ignored
        Assert.True(trie.Contains("ab"));
        // "a1b" contains "a", "1" (ignored), "b" -> becomes "ab" in trie
        // contains("a1b") also normalizes to "ab"
        Assert.True(trie.Contains("a1b"));
        Assert.False(trie.Contains("ac"));
    }

    [Fact]
    public void TestFinalizer()
    {
        void CreateAndForget()
        {
            var trie = new TrieNative(100, 3);
            trie.Insert("test");
        }

        CreateAndForget();
        GC.Collect();
        GC.WaitForPendingFinalizers();
    }
}
